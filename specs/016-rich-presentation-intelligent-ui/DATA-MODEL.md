# Data Model - Intelligent Workspace Presentation Runtime

**Status:** IN_REVIEW, revision 0.4 design proposal. RichPresentation stays immutable and message-bound. The product owner selected V1 Saved Views and directed a Feature 007 B-21 amendment; its Artifact representation is drafted and pending cross-owner review at T25. Optional answer-input persistence remains separate and requires its own approved migration and retention design (T22/B-46).

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
| block kinds | Existing profile: TEXT_SLICE, LAYOUT, CARD, CALLOUT, TABLE. Proposed V1 profile after promotion: these plus METRIC, CHART, SAFE_CALCULATOR and registered local input bindings (NUMBER, RANGE, SELECT). |
| citations, actions | empty |
| block_provenance | one per block path, origin SEMANTIC_MESSAGE or MODEL_INTENT, all ref arrays empty |
| METRIC | measure, finite value, units/currency/scale, time interval, aggregation, exact source binding and accessible label; ambiguous semantics refused |
| CHART | chart_kind BAR, LINE or AREA; binding to a semantic TABLE slice or exact authorized structured result; accessible_summary at most 2,000 |
| SAFE_CALCULATOR | inputs at most 8 (id, label at most 200, initial value bound to a table cell), formula_ast at most 64 nodes and depth 12, output_format NUMBER, INTEGER, PERCENTAGE or CURRENCY_USD, fallback_text at most 2,000 |
| Local input bindings | Proposed primitive numeric/range/enum values only; closed value bounds, exact declared state keys and no direct action/IPC binding; requires B-46 and schema promotion |

## 3. Lifecycle

A RichPresentation is immutable and uniquely bound to one committed message. The trusted publisher allocates and retains its publish identity; replay uses that identity and digest. A binder upgrade does not create a second presentation for the same message. A conversational modification creates a new message and optional presentation; the original remains unchanged. Rejected candidates are not stored; any `rich.presentation.rejected.v1` event requires T02 promotion and contains ids and reason only. A partial provider stream never becomes a committed or persisted RichPresentation.

The presentation controller owns one in-memory input revision keyed to the exact Workspace/Conversation/message/presentation/document digest while that view is open. Inline and Canvas attach to this same controller; closing the view, switching Workspace, revocation or restart clears it. Under current accepted B-40, inputs reset after a full close/restart and are not persisted or transmitted. Proposed B-46 may authorize separate explicit host commands for remembered inputs or reviewed ordinary-turn sharing; the registered component itself never performs either operation.

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

The immutable RichPresentation record itself continues to use `rich_presentations`; any renderer-profile/schema version change requires a compatibility and fallback review. Feature 016 adds no SavedView storage record: the proposed Library representation is an ordinary Feature 007 ArtifactVersion and remains pending that owner's review. If T22 accepts remembered answer inputs, the migration must be allocated through the global migration owner and include backup, retention, deletion, revocation, quota, expected revision and receipt behavior. No migration number is reserved by this proposal. Unknown renderer/state schema versions fall back to semantic text and do not hydrate state.

## 7. Revision 0.4 proposed interaction records and lineage

The [pipeline LLD](../../docs/ACP-PRESENTATION-PIPELINE-LLD.md) sections 6-9 define the proposed fields, partition key, revision/idempotency/deletion fences, quota, consent, privacy and recovery. These are TARGET only; no table, migration number or served operation is allocated here. `PresentationInteractionState` stores only declared primitive inputs and consent under owner/device + Workspace + Conversation + message + presentation + document digest + renderer/state schema version. Derived values are recomputed; source document and message are immutable. Retention follows parent deletion and authorization, and Forget must fence late saves. Backup policy, encryption, quota and deletion-receipt retention are T22 gates.

Transient controller state is shared between inline and expanded Canvas and discarded when the view closes or on restart under B-40. Persistence is separately opt-in and never implies sharing with the agent. If accepted, a modification request explicitly snapshots selected inputs and base revision into a new Conversation turn; new-message provenance records parent presentation identity/digest and base state revision. State is not automatically migrated across document/version namespaces.

### 7.1 Saved-answer identity boundary (TARGET proposal; A-103)

- **Conversation answer:** the committed semantic message and its immutable RichPresentation remain the source of truth. Where a presentation exists, reopening the Conversation reads that exact message-bound projection; it does not create a separately named Library item. The current code has no production publisher. Under accepted B-40, calculator inputs return to their original values after unmount/restart.
- **Remembered inputs:** `PresentationInteractionState` is a proposed local, device-scoped input record only. It is not an app package, ArtifactVersion, reusable view identity, or Library entry. It stays unavailable until T22 and its owner-approved persistence, deletion and recovery contract are accepted and implemented.
- **Reusable interactive deliverable:** a generated app can appear in Library only as a committed ArtifactVersion through the Feature 007 Artifact owner path, with exact source/provenance and a durable create receipt. Feature 007's create route is not currently served; the interactive app producer/runtime is not implemented or qualified. Even when published, V1 execution remains PREVIEW_ONLY.
- **Running-app data:** the optional app-data store described by [Interactive Artifact Runtime](../../docs/INTERACTIVE-ARTIFACT-RUNTIME.md) §9 belongs to that isolated Runtime, is keyed by Workspace/Artifact/version, and has its own consent/deletion/qualification gate. It is separate from Feature 016 `PresentationInteractionState`, cannot create or update an Artifact, and remains unavailable while the Runtime is PREVIEW_ONLY.

The product owner directed on 2026-11-17 that Saved Views remain in V1 and Feature 007 B-21 be amended. Feature 007 revision 1.1 proposes a Library-backed **Saved View** as an ArtifactVersion with exact source Conversation/message/presentation IDs and digests, a semantic fallback, and the canonical RichPresentation snapshot. It has no user-selected session-state snapshot: accepted B-40 keeps changed calculator values transient and never sends them. Save and update are explicit user operations; update appends a new immutable ArtifactVersion from a new committed eligible presentation. It is not executable code, not the original message, and not a second UI-tree schema. The amendment remains TARGET/IN_REVIEW pending cross-owner review and Feature 016 T01/T02.

If Feature 007 owner review finds that this contract weakens Artifact invariants, do not implement it through an improvised side table; return to the product owner before T01 and revise the V1 promise. Do not add a competing UI schema or infer a Library object from T22 input persistence. V1 uses Archive, not hard Delete, for Saved View removal.

### 7.2 State-owner separation

| State | Owner | Lifetime and persistence |
|---|---|---|
| Current inline/Canvas input state | Feature 016 desktop controller | Local and transient by default; shared only between surfaces for the exact presentation digest |
| Remembered answer inputs | Feature 016 + Storage, after T22 | Explicitly consented, device-local, exact-message/digest namespace, expected revision and durable receipt; clear/fence on Forget, revocation, Workspace/Conversation deletion |
| Saved View snapshot and version | Feature 007 ArtifactStore/Library, after T25/T01/T02/T23 | Explicit create/update; immutable ArtifactVersion with source provenance and owner receipt; open reauthorizes exact version and restores declared defaults; session-modified values are never persisted under B-40 |
| Executable Artifact app data | Interactive Artifact Runtime | Separate per-Workspace/Artifact/version store; remains unavailable while Runtime is PREVIEW_ONLY |

No state is silently sent to the selected native agent. Conversational modification previews and sends a selected snapshot through an ordinary new Conversation turn; that creates new message lineage, not an in-place presentation mutation.
