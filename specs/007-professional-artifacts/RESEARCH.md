# Research - Professional Artifacts One Format

**Feature:** 007 | **Checked:** 2026-11-06 (active source, composed routes, storage, Tauri bridge and UI)

## Sources inspected

- Normative owners: `docs/ARTIFACTS-EVIDENCE.md`, `docs/PRESENTATION-RUNTIME.md`, `docs/INTERACTIVE-ARTIFACT-RUNTIME.md`, `docs/V1-PRODUCT-RELEASE-CONTRACT.md` area 9, `docs/EVENTS.md`, `docs/schemas/domain-event.schema.json`, `docs/API.md`.
- Backend: `apps/litecoworkd/src/artifact_operator.rs`; `crates/storage-core/src/artifact.rs`; `crates/storage-sqlite/src/artifacts.rs`; `apps/litecoworkd/src/environment_office_qualification.rs`; `apps/litecoworkd/src/zip_intake_operator.rs`; `capabilities/zip_intake/README.md`.
- Native bridge and UI: `apps/litecowork-ui/src-tauri/src/artifact_bridge.rs`; `apps/litecowork-ui/src/artifacts/ArtifactLibrary.tsx`, `ArtifactWorkbench.tsx`, `TaskArtifactOutputs.tsx`, `StructuredTextPreview.tsx`, `artifact-api.ts`, `desktop-artifact-api.ts`, `README.md`; `apps/litecowork-ui/src/App.tsx` Library/Work integration.
- Machine route inventory: `docs/schemas/served-route-inventory.json` checked against `artifact_operator.rs` route composition.

## Source and route findings

- Artifact domain models and immutable version records exist. The active read store has Artifact list/get, exact version read and bounded content read. The Library write store handles promote/archive. The version-write store handles idempotent append only; there is no `create_artifact` method in the production Storage traits or `SqliteWorkspaceStore` implementation.
- The active daemon mounts GET list, saved-only Library, get, version metadata/content, text edit head, promote, archive, text-version append and Task artifact read. It does not mount `POST /v1/artifacts` or any general Artifact create/import-convert operation.
- `artifact.created.v1` and the intended atomic create contract appear in canonical event/schema docs and `ARTIFACTS-EVIDENCE.md`, but repository search found no active production producer. The one production `ArtifactRecord` constructor in `artifact_operator.rs` clones an already existing Artifact during text-version append; existing store test Artifacts are seeded directly in SQL. Do not report create support based on record/type/schema existence.
- The active append endpoint accepts only owner-authored UTF-8 `text/plain`, maximum 1 MiB, on an existing managed Artifact. It pins the Artifact aggregate version (`If-Match`), content version, backing Resource aggregate version and exact parent ResourceRevision; `Idempotency-Key` replays a matching receipt. Storage verifies the staged blob digest/size/media/UTF-8 and atomically appends the ResourceRevision and immutable ArtifactVersion, events, state snapshots, provenance and receipt. External-resource content is refused.
- The append route stamps user-authored provenance (`user.text_edit`), has no source inputs, no verification refs and no Attempt ID. That is appropriate for a manual text edit; it is not the Artifact publication route required for generated Task deliverables.
- Library promotion and archive are real metadata-only transitions with version/receipt checks; they never create bytes or Artifact versions. `listLibrary` returns SAVED only; the main Library component exposes Saved, Task outputs (TRANSIENT) and Archived status filters.

## Format, editor and export findings

- `supportsTextPreview()` allows plain text, Markdown, CSV/TSV, JSON, XML and YAML variants. The preview is bounded to 1 MiB and uses a restricted safe renderer/CSV parser; it never executes HTML or arbitrary model-provided JavaScript. This is a display capability, not editor capability.
- The current editor/head/append path is only managed `text/plain` up to 1 MiB. Markdown/CSV/JSON/YAML/XML previews do not mean those formats can be edited and round-tripped.
- Native Save As is a genuine exact-version local operation for managed blobs up to 10 MiB. The Tauri bridge re-reads Artifact metadata, selected immutable version and backing ResourceRevision; checks content digest/size/media/storage-ref; opens a native picker; fetches exact content from authenticated Operator; validates response type/length/digest and writes atomically through a same-directory staged file/rename. Cancelled picker fetches no content and touches no destination. Only SAVED/CANCELLED returns to WebView.
- Native Save As creates no Artifact/Resource revision or domain event and no durable ExportReceipt. It must not be described as a durable provider/server export or evidence that an export receipt has been committed.
- Linked external Artifact content is not fetched through Save As; the current content resolver returns PROVIDER_UNAVAILABLE until provider resolution is qualified. Historical managed text can be restored into a draft and published as a new version; `restored_from` provenance is not stored.
- Office qualification code observes whether a format/provider/runtime environment might support DOCX/XLSX/PPTX/PDF; it is not a parser, editor, exporter or verified Office round-trip. No active general Office document engine dependency or format adapter was found. ZIP intake is source-build/Linux-only, does not publish extracted members as Resources/Artifacts, and explicitly rejects ZIP-based Office containers in its conservative path.

## Desktop UI crosswalk

- `ArtifactLibrary` lists exact committed records, paginates and opens a `ArtifactWorkbench`; it has no create/generate/import-convert action. A clean/fresh Workspace may contain no Artifacts.
- `TaskArtifactOutputs` is a read-only consumer of already committed Task outputs. It does not create them. Feature 005 currently has no plan/Attempt/Environment dispatch path, so no end-to-end Task-generated Artifact path exists.
- `ArtifactWorkbench` supports exact-version preview/history/compare, a recent-ten-version history panel (exact older version can be requested directly), bounded line diff, provenance display, restore draft for managed text/plain, Library promote/archive and native Save As. It is a viewer/version manager with narrow text editing, not a professional office authoring suite.
- Resource upload accepts file formats including DOCX/XLSX/PPTX/PDF but creates Workspace Resources. The Library labels them as Resources and provides preview/no-preview or Save original based on content type and size. It does not convert these imports into Artifact records.
- The UI provenance panel exposes source Task/attempt/inputs/transformation/verification references where present but does not claim that it evaluated their assurance. Semantic answer fallback and registered safe renderers are separate Presentation requirements, not an artifact format editor.

## Product and architecture assessment

- The core immutable Artifact/Resource versioning and digest-bound local text append are technically coherent and useful. The native exact-version Save As path has strong identity/digest checks and does not leak bytes or filesystem destinations into WebView IPC.
- The V1 outcome is still missing. A user cannot create the first Artifact through the production Artifact owner API, cannot invoke an accepted professional editor/round-trip engine, and cannot commit the specified export receipt. The current path can inspect and manage an Artifact that some other producer has already inserted.
- Feature 005 is an upstream dependency for Task-generated output; Feature 003 is an upstream dependency for admitted Resource inputs. Do not claim a full deliverable workflow when those producer paths remain incomplete.
- B-21, B-30 and B-31 must be separately resolved and promoted into the canonical owner contracts/schemas before implementation. Do not assume DOCX/XLSX support because file upload accepts their extensions or a host readiness probe recognizes their media types.

## Alternatives and decisions

- Preview every common office format and call it supported: rejected. Format support must be independently declared viewable/editable/exportable/unsupported.
- Treat Resource uploads as Artifacts: rejected. Resource and Artifact have different ownership, version, provenance and lifecycle contracts.
- Treat native Save As as the target export receipt: rejected until Product explicitly accepts that narrower local-export contract and durable receipt semantics are added.
- Build a second artifact database or write path inside the React UI: rejected. ArtifactStore remains the owner; create/append/export commands must pass through authenticated Operator and canonical storage transactions.

## Limitations

- No professional format engine, external provider, format round-trip, persisted export receipt or end-to-end Task-generated output was executed during this audit.
- No rendered USER acceptance was performed for create/edit/export/reopen. This is source/route/UI inspection, not qualification.
- Rival product pages were not re-fetched for this source audit; this feature's technical blocker decisions do not depend on competitive claims.

## Unresolved questions

See SPEC §5 blockers B-21 (format/engine), B-30 (initial Artifact creation path/atomic owner transaction), and B-31 (export receipt and distinction from native Save As).
