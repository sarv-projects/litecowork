# UI behavior - Professional Artifacts One Format

Behavior, not pixels. Screen inventory: [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md). Control classes: [V1-SETTINGS-CONTRACT-LEDGER.md](../../../docs/V1-SETTINGS-CONTRACT-LEDGER.md) and [SEMANTIC-SERVED-UI-MAP.md](../../../docs/SEMANTIC-SERVED-UI-MAP.md). Action states: [V1-UI-ACTION-STATE-LEDGER.md](../../../docs/V1-UI-ACTION-STATE-LEDGER.md).

## Target user journey

1. The owner creates an Artifact through a served owner command or receives a Task-produced Artifact with exact Task/Attempt/provenance identity. Importing a Resource is not by itself Artifact creation.
2. Library shows committed Artifact identity, format/media type, current immutable version, exact ResourceRevision, provenance, inputs and verification references. SAVED, TRANSIENT and ARCHIVED remain distinct.
3. The format adapter advertises the exact operations it supports: view, edit, validate, compare, export and reopen. Unsupported operations are absent or explicitly unavailable; they are not generic buttons.
4. Editing opens the exact version and produces a draft based on a pinned head. Publishing appends a new immutable ArtifactVersion/ResourceRevision after the format validator and content digest succeed. Stale head preserves the draft and requires an explicit rebase/retry.
5. Export selects an exact version and destination class, runs the format engine outside SQLite, validates exported bytes, and records a separate ExportReceipt. Native Save As is labeled as a local file save and is not presented as that receipt.
6. Closing and reopening LiteCowork loads exact committed version bytes and provenance. Version compare uses stored exact immutable versions, not an unsaved buffer.
7. If rich presentation fails, the semantic source text or exact artifact source remains accessible; interactive app execution remains disabled unless the separate Artifact Runtime qualifies it.

## Current implementation crosswalk (not the target)

- Library is mounted under the Library route. `ArtifactLibrary` lists only committed records; the status selector distinguishes Saved, Task outputs (TRANSIENT) and Archived. It supports refresh, pagination and opening an exact Artifact. It has no create/generate/import-convert control.
- `TaskArtifactOutputs` is a read-only consumer of committed Task output records. Feature 005 currently has no working Task dispatch/Attempt/Artifact creation pipeline, so this component does not prove Task-generated output exists.
- `ArtifactWorkbench` supports exact version metadata/content reads, a recent ten-version history panel (direct exact version numbers can address older committed versions), bounded text compare/line diff, provenance inspection, promote/archive and Save As.
- Text preview up to 1 MiB accepts plain text, Markdown, CSV/TSV, JSON, XML and YAML variants through a bounded safe renderer. Preview does not authorize editing. The production append/edit-head route accepts only managed `text/plain` up to 1 MiB; historical text can be restored as a new version, with no dedicated `restored_from` relation.
- Native Save As is available for managed blobs up to 10 MiB. The Tauri bridge re-reads exact Artifact/version/ResourceRevision, verifies media type/size/digest and writes atomically through a native picker. It returns SAVED/CANCELLED only, stores no export receipt/event, and does not access linked external content.
- Uploading `.docx`, `.xlsx`, `.pptx`, `.pdf` or other media in Library creates a Resource, not an Artifact or an editable document. `environment_office_qualification.rs` contains Office-format readiness observations, not editor/parser/exporter implementations. ZIP Office containers are not unpacked by the current ZIP intake slice.
- No production create Artifact call site or `artifact.created.v1` producer was found. Fresh Library can therefore be empty. A general format-specific professional authoring/export flow is not present.

## Required rendered regression cases after implementation

- Empty Library distinguishes no committed Artifact from read failure; create/import/source conversion appears only after its handler and receipt exist.
- A created Artifact shows exact version 1, backing ResourceRevision, content digest, provenance/input refs and creation receipt after reload/restart.
- For the chosen format: edit, format validation, save, export, close, restart, reopen, exact-version compare and export receipt replay.
- Unsupported PDF/Office/image/external-resource operations show read-only/unavailable, not an enabled editor or fake success.
- Stale Artifact/Resource head preserves the draft; identical idempotent retry replays the same receipt; changed-payload key reuse conflicts.
- Native Save As says “saved locally” and never shows a provider export receipt. Cancelled Save As fetches no content and changes no destination.
- Rendering failure retains semantic content/source and an accessible text fallback.

## Validation

Source-level storage tests and an exact text append route do not qualify one professional format. Success requires the selected engine/version, format-specific fidelity checks, committed version/receipt, native rendered USER path and reopen after restart on the declared OS.
