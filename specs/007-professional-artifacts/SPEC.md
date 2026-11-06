# 007 - Professional Artifacts One Format

**ID:** 007
**Status:** DRAFT
**Owners:** Artifacts, Presentation, Storage, Desktop UI
**Updated:** 2026-11-06
**Revision:** 0.3
**Target release:** V1 local desktop
**Dependencies:** root ARCHITECTURE.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md, docs/IMPLEMENTATION-MASTER-PLAN.md, docs/ARTIFACTS-EVIDENCE.md, docs/PRESENTATION-RUNTIME.md, Feature 003 Resource admission, Feature 005 Task execution for Task-generated outputs, and the owner documents named in RESEARCH.md. Feature 001 remains the Conversation send authority.

## 1. Problem

Library cannot claim editing it does not have. Semantic answer text must survive if rich rendering fails. Interactive apps are a separate runtime.

Observable outcome: The owner creates, revises, exports and reopens one professional deliverable and can see provenance and version differences.

## 2. Goals

- Persist Artifact identity, immutable versions and provenance.
- Qualify one format for real edit, save, export and reopen across restart.
- Show version differences and source provenance.
- Keep semantic Conversation content when presentation fails.

## 3. Non-goals

- Every office format in one slice.
- Interactive artifact execution (separate runtime; inert until qualified).
- Using preview of one format as proof of editing another.

## 4. Current evidence

The active checkout has a real Artifact read/Library/version model and a narrow immutable text-version append path. It does not implement general Artifact creation/publication, a format-specific professional editor, or a durable export-receipt flow. The matrix below is a source/route/UI crosswalk, not owner acceptance or release qualification.

| Capability | Current source/route evidence | Limitation / release meaning |
|---|---|---|
| Artifact identity and immutable versions | CODE: `ArtifactRecord`, `ArtifactVersionRecord`, Resource/ResourceRevision correspondence, immutable content/provenance fields, dependency edges and read/write storage traits exist | Production daemon/store has no Artifact create method or route. Existing Artifact rows in tests are seeded directly; a fresh install may have no Artifact records. |
| Artifact create and generation | Canonical `docs/ARTIFACTS-EVIDENCE.md` defines `ArtifactStore.create()` and atomic creation of Artifact + ARTIFACT Resource + revision 1 + version 1 + events | TARGET only in this checkout. No `POST /v1/artifacts`, Artifact creation operator, or production `artifact.created.v1` event producer was found. `artifact.created.v1` exists in the schema/events registry but is not emitted by the active producer path. |
| Artifact version append | CODE/SERVED: `POST /v1/artifacts/{artifact_id}/text-version`; storage stages and verifies UTF-8 text, then atomically appends ResourceRevision + ArtifactVersion + events + idempotency receipt | This is a user-authored `text/plain` append to an already existing managed Artifact, capped at 1 MiB. It is not a general generated-output/format adapter, and it rejects external-resource content. |
| Library lifecycle | CODE/SERVED: list all, list SAVED Library, promote TRANSIENT→SAVED, archive SAVED→ARCHIVED; expected aggregate version and idempotency receipt | No create button or conversion from uploaded Resource to Artifact exists in the Library UI. Promotion/archive change metadata only; they do not create or export content. |
| Exact-version read and provenance | CODE/SERVED: artifact list/detail, exact version metadata/content, Task output list, input refs, provider/provenance, Attempt and verification references | Evidence refs are shown as references; this UI does not evaluate assurance. A missing verification ref is not verified output. |
| Preview support | UI previews bounded (≤1 MiB) managed text formats: plain text, Markdown, CSV/TSV, JSON, XML and YAML variants; restricted CSV/Markdown rendering is used | Preview is not edit support. PDF/Office/image content does not become editable because the Library accepts/imports it as a Resource. External linked content remains unavailable until provider resolution. |
| Editing and restore | CODE/UI: edit only managed `text/plain` Artifacts ≤1 MiB, optimistic head checks, draft preservation/rebase on conflict, idempotent publish; historical text can be restored as a new immutable version | No DOCX/XLSX/PPTX/PDF/HTML editor or structured document round-trip exists. Markdown/CSV/JSON can be previewed but are not enabled in the text edit route. `restored_from` provenance is not persisted. |
| Native Save As | CODE/UI: save exact selected MANAGED_BLOB version ≤10 MiB, revalidate Artifact/version/ResourceRevision/digest/media/size, native picker, authenticated content read, atomic same-directory write | This is a local native Save As/download, not the target export API. It creates no new Artifact/Resource version, domain event or durable ExportReceipt. Cancelled picker fetches no bytes/touches no destination. Linked external content is not saved through this path. |
| Export receipt | Canonical target asks for an export receipt distinct from version commit | No export receipt record, event, Operator route or UI history was found. The UI reports local Save As as SAVED/CANCELLED only. |
| Task-produced outputs | `GET /v1/tasks/{task_id}/artifacts` and `TaskArtifactOutputs` render committed output records read-only | Feature 005 currently cannot dispatch a Task/Attempt/Environment or create the general Artifact output. This UI is a consumer, not an Artifact generator. |
| Office/format engine | `environment_office_qualification.rs` has format-specific capability/OS observations for DOCX/XLSX/PPTX/PDF | These are readiness probes, not parsers/editors/exporters. No active DOCX/XLSX/PPTX/PDF editing engine or verified round-trip was found. ZIP intake is source-build/Linux-only, not Office extraction/publication. |
| Library and Workbench UI | `ArtifactLibrary` lists SAVED/TRANSIENT/ARCHIVED records; `ArtifactWorkbench` versions/compares/previews/edits text/promotes/archives/Saves As. Work Task outputs opens the same Workbench read-only | The current UI is a viewer/version manager with narrow user text editing. It cannot originate an Artifact or author a professional Office/PDF deliverable. |

Feature 007 remains DRAFT. The stated journey (create, revise, export, reopen one professional deliverable) is not implemented end to end: Artifact creation, one accepted format engine, and a durable export receipt are missing. Existing immutable version, metadata, narrow text-edit, Library and native Save As features are real but narrower.

## 5. LOCKED and FLEXIBLE decisions

### LOCKED

- ArtifactStore is the owner of bytes and versions (ARTIFACTS-EVIDENCE).
- Presentation does not own persistence (PRESENTATION-RUNTIME).
- Format support must be declared viewable, editable, exportable or unsupported (release contract).
- Provenance links Conversation or Task and source revisions.

### FLEXIBLE

- Module file layout and private helper names, provided the transactions and secret boundaries stay intact.
- Presentation copy, provided state distinctions remain visible.

### Open decisions

- B-21: Which single professional deliverable format is first? Choose one that has a real parser/editor, deterministic save/export, exact reopen/read-back, format validation and an independently testable round-trip. Current text/plain editing is not a professional document editor. Preview support for Markdown/CSV/JSON/YAML/XML does not satisfy editability.
- B-30: Who creates the initial Artifact and under which authenticated command/transaction? Define user-authored, imported, provider-linked and Task-generated Artifact paths without treating a Resource upload as an Artifact. The create transaction must bind the Artifact, ARTIFACT Resource, first ResourceRevision, version 1, provenance/dependency edges, canonical events, snapshot and idempotency receipt atomically.
- B-31: What is the target export contract? Distinguish native local Save As (a user-selected destination, no domain event) from a durable export receipt (exact Artifact/version, export format/transformer version, destination class, content digest, actor, timestamp and replay identity). Decide whether the initial V1 slice requires a persisted export record, and ensure no raw local path is written into shared events.

### Blockers

- B-21: Product, Artifacts and Desktop UI must select one format and name the exact supported editor/parser/export engine and version, OS/build requirements, supported operations (view/edit/export/reopen/compare), fidelity/round-trip criteria, size bounds and failure behavior. No engine or format is inferred from an installed Office-readiness probe.
- B-30: Artifacts, Storage and Task owners must accept the initial Artifact creation authority and atomic create contract, including the Task/Attempt provenance rules, uploaded/imported-vs-generated distinctions, first-version content staging, immutable identity and event/receipt ordering. The current Artifact store has no create method/route.
- B-31: Artifacts, Storage, Desktop UI and Security must accept the export semantics and receipt owner. Define native Save As separately from server/provider export, pin exact ArtifactVersion and content digest, persist only approved destination metadata, and make retry/restart behavior idempotent. Current Save As has no durable export receipt.

## 6. High-level design

Target: UI Library commands call ArtifactStore through the authenticated Operator. A distinct create operation publishes the initial Artifact/ARTIFACT Resource/revision/version and provenance atomically. Subsequent versions are immutable. A format adapter performs bounded edit/round-trip and deterministic export outside SQLite; the exact output is digest-verified before the export receipt is committed. Native desktop Save As is a separate local file operation and is not substituted for a provider/durable export receipt. Presentation renders committed projections only. None of the create or durable export receipt path is served in the current checkout.

Trust boundary: the desktop UI is untrusted; the authenticated Operator is the only command path; native credentials and OS handles stay outside public events; SQLite transactions do not perform network or process calls.

## 7. Low-level design

Follow ARTIFACTS-EVIDENCE ArtifactStore and Library commands. First commit an initial Artifact under the B-30 create contract. The B-21 format adapter must implement the exact declared read, edit, validation, save, export and reopen path; publish new immutable versions only after the content blob and digest are verified. The B-31 export boundary creates a receipt distinct from the Artifact version. Unsupported media/operations return a typed unsupported error rather than a silent no-op. Existing managed text/plain append is a separate narrow fallback, not the selected format editor.

Ordering: authorize, compare expected version, commit the durable receipt, then perform any external call, then reconcile. Proposed payload fields are PROPOSED until the canonical owner document and machine schema change together.

## 8. Data

See [DATA-MODEL.md](DATA-MODEL.md). This feature does not create a second source of truth beside the domain owner.

## 9. Wiring

See [WIRING.yaml](WIRING.yaml). Requests enter through the Operator. External calls sit outside transactions.

## 10. Security

Artifacts are Workspace-scoped. Untrusted presentation cannot invoke host actions. Export paths stay inside authorized roots.

## 11. Errors and recovery

Crash during save rolls back the uncommitted version. Reopen loads the last committed version. Failed export does not change the current version pointer.

Races use compare-and-swap or a single-writer occurrence identity. Timeout after an external effect reconciles. Duplicate retries return the original receipt when the idempotency key matches and fail when the payload conflicts.

## 12. Crash and restart

Reload the last committed owner records. Do not infer success from an intent. Reconcile in-flight external work before a new side effect. Cancellation fences late results.

## 13. Configuration and compatibility

One format. Other formats may be read-only or hidden. Interactive run stays disabled.

OS qualification is per native desktop. WSL2 does not qualify Windows or Linux GUI behavior.

## 14. Observability

Log ids, versions, statuses and denial reasons. Do not log secret bytes, native handles, private locators or raw credentials. Audit commits include actor, workspace and receipt id.

## 15. EARS acceptance

- **AC-001:** WHEN the owner saves, THE SYSTEM SHALL commit a new immutable Artifact version or leave the prior version current.
- **AC-002:** WHEN the owner reopens after restart, THE SYSTEM SHALL load the last committed version bytes and provenance.
- **AC-003:** WHEN two versions are compared, THE SYSTEM SHALL show differences from stored versions, not from an unsaved editor buffer alone.
- **AC-004:** IF presentation rendering fails, THEN THE SYSTEM SHALL still show the semantic artifact text or source.
- **AC-005:** IF the format does not support editing, THEN THE SYSTEM SHALL label it unsupported or read-only and SHALL NOT claim save.
- **AC-006:** WHEN export succeeds, THE SYSTEM SHALL record an export receipt distinct from the version commit.

## 16. Executable acceptance command

```bash
python3 .specify/scripts/validate_feature_package.py specs/007-professional-artifacts
```

Further gates after ACCEPTED/LOCKED are the cargo filters named in TASKS.md. They are not evidence until run on a recovered source tree.

## 17. Out-of-bounds

- Do not enable interactive artifact run.
- Do not treat PDF preview as DOCX editing.

## 18. Assumptions and open questions

- B-21, B-30 and B-31 are independent blockers. A format decision does not create an Artifact create route or export receipt by itself.
- Evidence verifier beyond deterministic content/hash and format-structure checks is out of the first one-format slice unless the selected format requires it; no artifact is described as VERIFIED merely because the bytes hash correctly.
- Feature 005 Task execution is a dependency for Task-generated Artifact output. The V1 Library must still have an explicitly supported user-authored/imported creation path if Task output remains unavailable.
- Native Save As is not required to create a durable ExportReceipt and must remain visibly distinct unless Product explicitly decides that the initial V1 export contract is satisfied by a native local export receipt with durable, safe metadata.

Each blocker has the owners named above. Resolution gate: owner decision recorded in this SPEC before status leaves DRAFT. Implementation tasks: 007-T01, 007-T02, 007-T03, 007-T04.

## 7a. Owner facts and implementation crosswalk checked 2026-11-06

Read from ARTIFACTS-EVIDENCE, PRESENTATION-RUNTIME, and INTERACTIVE-ARTIFACT-RUNTIME.

- An Artifact moves only TRANSIENT to SAVED by explicit promotion and SAVED to ARCHIVED by archive. Publishing to an ARCHIVED Artifact fails with ARTIFACT_ARCHIVED. Concurrent publication fails with STALE_VERSION. listLibrary returns only SAVED artifacts.
- The same Idempotency-Key and payload returns the recorded result. The same key with a changed payload returns 409 CONFLICT.
- The local text-append route accepts only managed text/plain up to 1 MiB and pins If-Match, content version, Resource aggregate version, and exact parent ResourceRevision. It rejects archived Workspaces, archived Artifacts, external-resource content, and HTML.
- Desktop Save As operates on one MANAGED_BLOB version up to the 10 MiB IPC bound. Success creates no Artifact or Resource revision and no domain event. Destination bytes do not return through WebView IPC.
- Evidence level is REPORTED, OBSERVED, or VERIFIED. REPORTED cannot satisfy an OBSERVED or VERIFIED criterion. The Runtime append path admits only REPORTED Evidence.
- An interactive run is volatile: PREVIEW_ONLY, STARTING, RUNNING, STOPPING, STOPPED, FAILED, REVOKED, or INTERRUPTED. It does not increment Artifact.version, does not start automatically, and does not run an ARCHIVED Artifact. Unproved quiescence is INTERRUPTED, not STOPPED.
- RichPresentationAvailability other than AVAILABLE still renders the committed semantic message. Presentation payloads forbid raw HTML, scriptable SVG, inline handlers, CSS, and arbitrary JavaScript. PresentationStatus COMPLETE does not mark a Task complete.
- Current source routes include list/get/version/content, edit-head, bounded text-version append, promote/archive, Task Artifact read and Library read. There is no createArtifact or export receipt operation in the composed router. Do not claim the OpenAPI owner target is served until the daemon route inventory contains its handler.



## 7b. Current source and visible Library boundary — 2026-11-06

- Production Artifact storage exposes list/get, exact immutable version metadata/content, bounded edit-head/text append, promote and archive. There is no production `create_artifact` storage method or served `POST /v1/artifacts` route. The canonical `artifact.created.v1` event and atomic-create contract are target documents, not an active producer.
- The active Library UI has Saved, Task outputs (TRANSIENT) and Archived views. It lists committed records and opens `ArtifactWorkbench`; there is no create/generate/import-convert action. Task outputs are read-only projections of already committed outputs; Feature 005 does not yet produce them through a running Task.
- `ArtifactWorkbench` supports exact-version preview/history/compare, a recent-ten-version panel, bounded text/plain draft editing and append (maximum 1 MiB), restore-to-draft for managed text/plain, metadata-only promote/archive and native Save As. The current editor path is not a Markdown/CSV/JSON/YAML/XML editor merely because those formats can be previewed.
- Preview supports bounded safe display of selected text formats; it does not execute HTML or model-provided JavaScript. Office qualification helpers observe potential host readiness; they are not DOCX/XLSX/PPTX/PDF parsers, editors, exporters or round-trip engines.
- Native Save As is a verified local-file write for one exact managed blob version within the 10 MiB bridge bound. It creates no Artifact/Resource revision, domain event or durable `ExportReceipt`. External linked content remains `PROVIDER_UNAVAILABLE`. Do not label this as server/provider export or a committed export receipt.
- Resource uploads can retain DOCX/XLSX/PPTX/PDF originals as Resources, but do not convert them into Artifact records. ZIP intake does not create Artifact members.
- No user-authored/imported Artifact creation path, professional format engine, generated Task Artifact producer or rendered end-to-end create/edit/export/reopen journey was qualified. B-21 (format engine), B-30 (initial creation transaction/path) and B-31 (export receipt semantics) remain independent blockers.

## 19. Requirement traceability

| Requirement | Spec | Canonical contract | Task | Acceptance |
|---|---|---|---|---|
| AC-001 | EARS acceptance | canonical owner docs cited in this SPEC | 007-T01 | AC-001 |
| AC-002 | EARS acceptance | canonical owner docs cited in this SPEC | 007-T01 | AC-002 |
| AC-003 | EARS acceptance | canonical owner docs cited in this SPEC | 007-T03 | AC-003 |
| AC-004 | EARS acceptance | canonical owner docs cited in this SPEC | 007-T04 | AC-004 |
| AC-005 | EARS acceptance | canonical owner docs cited in this SPEC | 007-T03 | AC-005 |
| AC-006 | EARS acceptance | canonical owner docs cited in this SPEC | 007-T02 | AC-006 |
