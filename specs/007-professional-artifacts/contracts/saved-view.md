# Saved View Artifact contract

**Status:** TARGET amendment for Feature 007 revision 1.1, written 2026-11-17 after the product owner's direction to keep Saved Views in V1 and amend B-21. This records the selected product direction and the proposed cross-owner contract; it does not claim Feature 007 revision 1.1 or Feature 016 revision 0.4 has completed every required owner review or implementation gate.

## 1. Product boundary and owners

A **Saved View** is a user-requested, reusable snapshot of one eligible committed RichPresentation in Library. It is owned by the existing ArtifactStore and represented by an Artifact with `kind = SAVED_VIEW` and media type `application/vnd.litecowork.saved-view+json`. It is a bounded data document, not a generated app, not a new UI tree, not the source Conversation message, and not an `INTERACTIVE_APP` package. Library is its only persistent product home; no Saved Apps navigation destination is added.

The semantic Conversation message remains authoritative for the original answer. A Saved View is an independent user-created copy with exact source lineage and a semantic-text fallback. It preserves only the presentation's original declared defaults, not calculator values changed during an open session. This preserves accepted Feature 016 B-40, which keeps calculator state transient and never persists or sends it. A Saved View can still reopen after its source Conversation is deleted because its content is self-contained. Deleting the source removes or marks the origin link according to Feature 014; it does not delete the saved Artifact.

Feature 016 owns presentation composition, the closed RichPresentation profile, input declarations, and Rust/TypeScript validation parity. Feature 007 owns the Saved View Artifact bytes, ArtifactVersion/ResourceRevision pairing, Library visibility, create/update receipts, versioning, and archive lifecycle. Core/Operator owns authorization and cross-store source validation. The desktop only proposes a typed action and renders committed owner projections. No provider, ACP agent, browser storage, or generated component owns persistence.

## 2. B-21 amendment

The accepted `text/markdown` choice remains the first **professional document** format with its existing parser, deterministic round trip, 1 MiB document input cap, and 10 MiB export cap. Saved View is one separately typed B-21 container exception, not a general permission for arbitrary JSON/media, and it does not pass through the Markdown editor or exporter.

The one additional V1 Artifact media type is `application/vnd.litecowork.saved-view+json`. Its canonical envelope is [saved-view-artifact.schema.json](../../../docs/schemas/saved-view-artifact.schema.json), embeds the existing canonical RichPresentation document, carries exact source IDs and digests, and includes the exact semantic answer text for fallback. Calculator definitions and original values already contained in the RichPresentation are preserved unchanged; session-modified input values are not copied. The full canonical UTF-8 JSON content is capped at 2 MiB; the embedded RichPresentation retains its 512 KiB cap and the semantic snapshot retains a 1 MiB UTF-8 cap. JSON must be RFC 8785 canonical before its Artifact content digest is committed.

JSON Schema `maxLength` counts Unicode characters, not encoded bytes. The Rust owner validator must enforce the semantic snapshot's 1 MiB UTF-8 byte bound and the complete envelope's 2 MiB encoded-byte bound after canonical serialization, in addition to schema validation. The shared TypeScript validator mirrors user-visible eligibility/limits but cannot authorize the write.

An Artifact is eligible only when all conditions hold:

- The source is a committed semantic message and an AVAILABLE, valid RichPresentation for that exact message, in the same active Workspace.
- The document uses a Feature 016 profile accepted for this release. Its root blocks, bindings, calculations, accessibility text, and producer provenance pass the same canonical Rust and TypeScript validation used for inline rendering.
- Every value and chart is self-contained in the saved envelope or derives from the embedded semantic message. For this V1 profile, live capability results, unresolved Resource/Artifact references, remote URLs, MCP Apps, arbitrary actions, external media, and executable app content make the view ineligible.
- The source `conversation_id`, `message_id`, `presentation_id`, semantic digest, and canonical RichPresentation digest match the durable owner records exactly. The semantic text is copied byte-for-byte from that committed message.
- The RichPresentation contains only its accepted component defaults, data and deterministic calculations. The Saved View envelope has no `input_snapshot`; no values are sent to an agent by saving or opening.

The desktop never sends arbitrary RichPresentation JSON as trusted Artifact content. It requests a snapshot by exact source IDs. The daemon reloads the committed message and presentation, validates their exact linkage and self-contained profile, constructs canonical bytes, and passes those bytes through the Artifact create/write path. Calculator values changed while a view is open remain transient and reset to the saved RichPresentation defaults when reopened. Persisting modified values or sending them in a Conversation turn requires a separate B-40/B-46 owner decision; this Saved View decision grants neither behavior.

## 3. Identity, persistence and lifecycle

No `SavedView` table, independent saved-view ID, browser localStorage record, provider memory record, or second Library is introduced. `ArtifactId` is the stable Saved View identity. Each save/update is an immutable ArtifactVersion paired with exactly one ResourceRevision on its backing ARTIFACT Resource. The ArtifactVersion content bytes contain the full saved-view envelope; its existing digest covers source metadata, semantic fallback and presentation. Provenance records the exact source lineage and schema/profile/renderer versions. The Artifact is not represented as a live dependency edge to its source message or Resource.

`Save to Library` is an explicit user request and creates the Saved View directly in `SAVED` status in the same B-30 transaction that creates the Artifact, backing Resource, revision 1, ArtifactVersion 1, provenance, snapshot, existing creation events and principal-scoped idempotency receipt. Direct `SAVED` creation is a narrow exception for this explicit operation; other Artifact creation keeps its existing status semantics. The create receipt is the only success signal. No second promote command is required, so a crash cannot leave a half-saved view hidden as TRANSIENT.

`Open` reads the exact selected immutable version. The daemon reauthorizes the Workspace and Artifact, validates the envelope and supported profile, and returns that exact version; it never follows a newer head implicitly and never dispatches the source agent. If the renderer/schema is unsupported or the rich part fails validation, show the stored semantic snapshot with a clear unavailable explanation; do not render stale or partial controls.

Changing a control changes only view-local state. It resets to the Saved View's declared defaults when the view closes or reopens; V1 has no `Save changes` action for local calculator values. Re-running the originating Conversation or asking the agent to change the answer creates a new message/presentation; it does not mutate this Artifact. The user may separately select `Update from answer`, review a new committed source in the same Conversation, and append an immutable ArtifactVersion/ResourceRevision. A stale head preserves the review draft. Updating from another Conversation creates a new Saved View to preserve source lineage. No modified calculator values are automatically included in an agent message.

`Archive` reuses the existing SAVED→ARCHIVED lifecycle and hides the item from the normal Library list while retaining its immutable history under existing retention/backup rules. V1 has no hard-delete or delete route for Saved Views. Archived content is not reopened for editing or appended. Retention/purge follows the Artifact owner's existing policy; a source Conversation deletion is not a request to purge a saved copy.

## 4. API and transaction contracts (TARGET)

All calls use the existing authenticated Operator, Workspace boundary, ArtifactStore, SQLite transaction, event registry and error taxonomy. These are target operations only until Feature 007 T01/T02 promotion and source handlers exist.

- `POST /v1/artifacts` adds `source.kind = RICH_PRESENTATION_SAVED_VIEW` with exact `conversation_id`, `message_id`, `presentation_id`, and source digests. The body does not accept arbitrary presentation, semantic text or calculator state. The daemon constructs the envelope from owner records and creates `kind = SAVED_VIEW`, the Saved View media type, and initial `library_status = SAVED` only for this explicit source kind.
- The create transaction authorizes the principal and active Workspace, verifies the committed message and exact presentation/digests, validates the eligible profile, stages the bounded canonical envelope, then atomically commits Artifact, ARTIFACT Resource, ResourceRevision 1, ArtifactVersion 1, provenance, snapshot, existing `resource.created.v1` / `resource.revision.observed.v1` / `artifact.created.v1` / `artifact.version.created.v1` events, and the existing idempotency receipt. No provider, filesystem, renderer or process call occurs inside SQLite.
- `POST /v1/artifacts/{artifact_id}/saved-view-versions` appends a new envelope from a newly committed eligible same-Conversation presentation. It pins `If-Match`, expected Artifact version, expected Resource version, exact parent ResourceRevision, exact source digests, and `Idempotency-Key`. A cross-Conversation update is a new Saved View, not a rewrite of existing lineage. Local calculator values are not included.
- `GET /v1/library?kind=SAVED_VIEW` (or an equivalent canonical filter chosen in T01) lists saved views within the authenticated Workspace. Existing exact Artifact/version/content reads open a pinned version. Existing `/archive` archives it. The UI must not expose an unfiltered list as if it were a separate store.
- Create and version append reuse the existing Artifact create/version receipts and canonical `artifact.created.v1` / `artifact.version.created.v1` event names. No SavedView-specific event, receipt table, or domain aggregate is added. Events carry bounded identity/version/digest metadata only, never the envelope, input values, semantic text, local path, provider credential, or native handle.
- `UNSUPPORTED_MEDIA_TYPE` applies to every non-Markdown/non-Saved-View Artifact write. Export, compare and native Save As of the internal Saved View envelope are unsupported in V1; the UI must not present the machine JSON as a user deliverable.

The source-validation race is resolved inside the same authoritative SQLite transaction as create/update. If Conversation deletion commits first, create/update returns canonical `NOT_FOUND` and writes nothing. If the Saved View transaction commits first, later Conversation deletion leaves the self-contained Artifact and masks/marks its origin according to Feature 014. For an update sourced from a new answer, a deleted or superseded source fails closed; the previous Saved View head remains unchanged.

## 5. User journey and state behavior

| Surface/control | Command and visible states | Result/recovery |
|---|---|---|
| Committed Conversation answer: `Open in Canvas` | Select the exact message/presentation; preserve scroll/composer and focus | Opens the same presentation in contextual Workbench; no new route, agent call, Task, app run or capture |
| Eligible answer: `Save to Library` | Disabled before the answer and presentation commit; absent for invalid, unsupported or externally backed views; pending/reconciling/saved/denied states | Sends exact source IDs only; the daemon creates the canonical snapshot and shows success only after durable create receipt; on ambiguity reconciles the same RequestId/body digest before retry |
| Library: `Saved views` filter within existing Library | Loading, empty, populated, denied, stale, archived | Lists saved Artifact IDs and committed versions; no permanent sidebar tab |
| Library item: `Open` | Reauthorize Workspace and exact version; validate type/schema/digest | Opens exact saved version; unavailable/corrupt/unsupported state retains no stale controls and offers source/fallback only when authorized |
| Saved View control change | Local transient state for this exact open version; accessible name, keyboard operation and reset | Does not persist, call IPC, send to agent, create Task/Effect/Approval or mutate source answer; reopen resets to saved declared defaults |
| Saved View: `Update from answer` | User selects a new committed answer in the same Conversation and reviews source/digest | Explicit append with new source lineage; cancel leaves both objects unchanged; another Conversation requires `Save as new` |
| Saved View: `Archive` | Confirm the exact Library object; busy/denied/archived | Existing archive receipt/status; no hard delete and no automatic source deletion |

At narrow widths the Canvas may become a full-screen surface, but it retains a clear return-to-chat path and the same object identity. Primary navigation remains uncluttered; Saved Views are a filter/collection inside Library, not another product mode.

## 6. Security, privacy and recovery

- The WebView is untrusted. Only typed source IDs cross IPC. Rust re-reads and correlates immutable source records before generating payload bytes.
- Workspace authorization is checked for both source and Artifact. Cross-Workspace source, stale presentation, mismatched digest, archived Workspace/Artifact and uncommitted message all fail before write.
- Content is canonical JSON validated against the Saved View envelope and the single canonical RichPresentation profile. Unknown versions/blocks/bindings fail closed. No eval, HTML/JS, model SVG, URL fetch, MCP UI, or `INTERACTIVE_APP` execution.
- Explicit Save warns that the semantic answer and declared RichPresentation defaults become durable Library content and follow Artifact backup/retention. Session-modified values remain transient; no background/autosave occurs.
- Lost response after commit is reconciled by the original owner-scoped receipt. Same RequestId with changed logical content returns `IDEMPOTENCY_CONFLICT`. Concurrent writers are fenced by both Artifact and Resource versions; only one head wins.
- Restart recovery reads committed ArtifactVersion/ResourceRevision pairs. It never uses an uncommitted WebView draft as success and never resurrects forgotten state through a side store. Corrupt bytes are unavailable and do not fall back to an untrusted serialized UI tree.
- Archive is supported; hard deletion, export of the machine envelope, source-independent agent sending, and cross-device input synchronization are not V1 Saved View behavior.

## 7. Acceptance and release gates

The following are TARGET acceptance cases for Features 007 and 016, not evidence that the behavior exists:

- Save from a committed exact message creates one SAVED Artifact, backing Resource, ResourceRevision 1 and ArtifactVersion 1 atomically; receipt replay is stable and changed payload conflicts.
- Cross-Workspace, stale-source, uncommitted-message, malformed envelope, unknown schema, unresolved resource binding, invalid input ID/range, oversized content and archived Workspace requests produce canonical `NOT_FOUND`, `STALE_VERSION`, `INVALID_ARGUMENT`, `UNSUPPORTED_MEDIA_TYPE` or `INTEGRITY_FAILURE` denial with no visible partial records/events.
- Two update attempts against one Artifact/Resource head commit exactly one immutable version; the loser keeps its draft and receives `STALE_VERSION`.
- Crash before commit leaves no view; crash after commit/before UI acknowledgement reconciles the original receipt; restart reopens the exact committed version and its declared defaults, not the previous session's transient input values.
- Delete Conversation before save denies the save; save before Conversation deletion retains the independent saved Artifact and shows an honest deleted-origin marker.
- Archive removes the view from the normal Library filter without hard deletion; existing Artifact retention/backup restore keeps exact version bytes, provenance and receipt behavior.
- Presentation rendering/schema failure leaves the complete saved semantic snapshot readable and never executes component data.
- Packaged desktop qualification must cover real UI → Tauri → authenticated Operator → authorization → ArtifactStore/SQLite → receipt/event → projection → restart on each release OS row, plus denial, replay, timeout-after-dispatch, cancellation/deletion race and recovery. Unit/schema tests alone are CODE, not INTEGRATED or QUALIFIED.

## 8. Required promotion sequence

1. Record the user's V1 direction in Feature 007's owner decision and A-103; keep the amendment itself IN_REVIEW until Artifact, Storage, Conversation, Privacy, Backup and Product owners review this exact contract.
2. Feature 007 T01 updates `ARTIFACTS-EVIDENCE.md`, its complete package, OpenAPI, canonical Saved View schema and `CreateArtifact` source classes; Feature 016 T01/T02 must promote the matching RichPresentation profile. Refresh UI/API/event/error schemas and generated types in that same contract-promotion work. The saved envelope contains no changed-state persistence.
3. Extend the existing Feature 007 create/version/capability/route/UI/recovery tasks with the exact file scope for Saved View; extend Feature 016 T23/T24 only after T01/T02 acceptance. No code, route, remembered-input persistence or state-sending behavior is enabled by this design decision alone.
4. Implement and qualify the full receipt/recovery/user journey before marking the V1 Saved View capability SERVED, INTEGRATED or QUALIFIED.
