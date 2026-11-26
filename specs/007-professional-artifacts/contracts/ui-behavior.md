# UI behavior - Professional Artifacts One Format

**Status:** Base revision 1.0 ACCEPTED/LOCKED; Saved View UI amendment IN_REVIEW after owner direction 2026-11-17. Optional numeric input snapshots have USER-DELEGATED design direction A-84 (2026-11-24), but cross-owner contract review and served handlers remain open. All Saved View controls below are TARGET and unavailable until typed owner handlers and receipts exist.

Behavior, not pixels. Screen inventory: [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md). Control classes: [V1-SETTINGS-CONTRACT-LEDGER.md](../../../docs/V1-SETTINGS-CONTRACT-LEDGER.md) and [SEMANTIC-SERVED-UI-MAP.md](../../../docs/SEMANTIC-SERVED-UI-MAP.md). Action states: [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md).

## Target user journey

1. The owner creates an Artifact through a served owner command or receives a Task-produced Artifact with exact Task/Attempt/provenance identity. Importing a Resource is not by itself Artifact creation.
2. Library shows committed Artifact identity, format/media type, current immutable version, exact ResourceRevision, provenance, inputs and verification references. SAVED, TRANSIENT and ARCHIVED remain distinct.
3. The format adapter advertises the exact operations it supports: view, edit, validate, compare, export and reopen. Unsupported operations are absent or explicitly unavailable; they are not generic buttons.
4. Editing opens the exact version and produces a draft based on a pinned head. Publishing appends a new immutable ArtifactVersion/ResourceRevision after the format validator and content digest succeed. Stale head preserves the draft and requires an explicit rebase/retry.
5. Export selects an exact version and destination class, runs the format engine outside SQLite, validates exported bytes, and records a separate ExportReceipt. Native Save As is labeled as a local file save and is not presented as that receipt.
6. Closing and reopening LiteCowork loads exact committed version bytes and provenance. Version compare uses stored exact immutable versions, not an unsaved buffer.
7. If rich presentation fails, the semantic source text or exact artifact source remains accessible; interactive app execution remains disabled unless the separate Artifact Runtime qualifies it.

8. Every view/edit/export/compare control reads the daemon capability matrix (SPEC §5). DOCX, XLSX, PPTX and PDF show a metadata card with "Read-only" or "Not supported in this version" and Save original; no editor or export button is rendered for them (AC-016).
9. Library offers "New Markdown document"; a committed Conversation message offers "Save as Artifact". The Artifact shows "From conversation" with a link, and the Conversation lists its Artifacts (AC-017, AC-018).
10. An eligible committed structured answer may expose **Save to Library** only after Feature 007/016 validation and its durable create handler are served. The control is absent for streaming/uncommitted messages, invalid/unsupported profile, unresolved external data, denied sources or stale digests. The user sees that the semantic answer and presentation will be saved. A separate unchecked-by-default **Include current numeric values** option saves only exact declared finite in-range/on-step number inputs as immutable ArtifactVersion content; values never enter events, receipts or an agent request.
11. Library presents Saved Views as a filter/group inside the existing Library, never a new persistent navigation tab. Open loads and reauthorizes the exact version; controls operate locally and restore that version's optional validated numeric snapshot or declared defaults. A distinct **Save current values** action appends an immutable ArtifactVersion only after explicit confirmation and receipt. **Update from answer** requires a same-Conversation committed answer and a review step; it clears old values unless exact input declarations remain compatible and the user opts in again. Another Conversation creates a new Saved View.
12. Saved View offers **Archive** through the existing Artifact lifecycle. It does not offer hard Delete, Export, generic JSON edit, or native Save As of its internal machine format. Unsupported buttons are absent or explicitly disabled with a reason.

## Current implementation crosswalk (not the target)

- Library is mounted under the Library route. `ArtifactLibrary` lists only committed records; the status selector distinguishes Saved, Task outputs (TRANSIENT) and Archived. It supports refresh, pagination and opening an exact Artifact. It has no create/generate/import-convert control.
- `TaskArtifactOutputs` is a read-only consumer of committed Task output records. Feature 005 currently has no working Task dispatch/Attempt/Artifact creation pipeline, so this component does not prove Task-generated output exists.
- `ArtifactWorkbench` supports exact version metadata/content reads, a recent ten-version history panel (direct exact version numbers can address older committed versions), bounded text compare/line diff, provenance inspection, promote/archive and Save As.
- Text preview up to 1 MiB accepts plain text, Markdown, CSV/TSV, JSON, XML and YAML variants through a bounded safe renderer. Preview does not authorize editing. The production append/edit-head route accepts only managed `text/plain` up to 1 MiB; historical text can be restored as a new version, with no dedicated `restored_from` relation.
- Native Save As is available for managed blobs up to 10 MiB. The Tauri bridge re-reads exact Artifact/version/ResourceRevision, verifies media type/size/digest and writes atomically through a native picker. It returns SAVED/CANCELLED only, stores no export receipt/event, and does not access linked external content.
- Uploading `.docx`, `.xlsx`, `.pptx`, `.pdf` or other media in Library creates a Resource, not an Artifact or an editable document. `environment_office_qualification.rs` contains Office-format readiness observations, not editor/parser/exporter implementations. ZIP Office containers are not unpacked by the current ZIP intake slice.
- No production create Artifact call site or `artifact.created.v1` producer was found. Fresh Library can therefore be empty. A general format-specific professional authoring/export flow is not present.
- Saved View controls and data types are TARGET only. No Saved View create/update bridge, Artifact kind, persisted envelope, Library filter, durable receipt flow or packaged journey is implemented or served.

## Required rendered regression cases after implementation

- Empty Library distinguishes no committed Artifact from read failure; create/import/source conversion appears only after its handler and receipt exist.
- A created Artifact shows exact version 1, backing ResourceRevision, content digest, provenance/input refs and creation receipt after reload/restart.
- For the chosen format: edit, format validation, save, export, close, restart, reopen, exact-version compare and export receipt replay.
- Unsupported PDF/Office/image/external-resource operations show read-only/unavailable, not an enabled editor or fake success.
- Stale Artifact/Resource head preserves the draft; identical idempotent retry replays the same receipt; changed-payload key reuse conflicts.
- Native Save As says “saved locally” and never shows a provider export receipt. Cancelled Save As fetches no content and changes no destination.
- Rendering failure retains semantic content/source and an accessible text fallback.
- Saved View: exact-source eligibility; committed receipt before success; stable replay; cross-Workspace/stale/uncommitted-source denial; defaults when no snapshot is saved; exact numeric declaration/digest/range/step validation when opt-in is selected; no partial snapshot application; explicit Save current values appends an immutable version; archive-only removal; update conflict draft preservation; restart exact-version reopen; source-Conversation delete race with retained self-contained content and masked provenance; malformed/unknown-schema semantic fallback.

## Validation

Source-level storage tests and an exact text append route do not qualify one professional format. Success requires the selected engine/version, format-specific fidelity checks, committed version/receipt, native rendered USER path and reopen after restart on the declared OS.
