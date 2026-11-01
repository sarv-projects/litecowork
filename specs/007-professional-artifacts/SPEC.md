# 007 - Professional Artifacts One Format

**ID:** 007
**Status:** DRAFT
**Owners:** Artifacts, Presentation, Storage, Desktop UI
**Updated:** 2026-11-01
**Revision:** 0.1
**Target release:** V1 local desktop
**Dependencies:** root ARCHITECTURE.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md, docs/IMPLEMENTATION-MASTER-PLAN.md, and the owner documents named in RESEARCH.md. Feature 001 remains the Conversation send authority.

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

This checkout is documentation-first. The behavior below is **TARGET**. Archived source and historical CODE notes are evidence of past work, not SERVED or QUALIFIED status on this tree. No owner has locked this package.

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

- Which single format is first: the owner must pick a format with a real editor, not a preview masquerading as edit.

### Blockers

- B-21: the one format and its edit/export engine.

## 6. High-level design

UI Library commands call ArtifactStore through the Operator. Versions are immutable. Export writes a derived byte package outside the version-commit transaction, then commits an export receipt. Presentation renders projections only.

Trust boundary: the desktop UI is untrusted; the authenticated Operator is the only command path; native credentials and OS handles stay outside public events; SQLite transactions do not perform network or process calls.

## 7. Low-level design

Follow ARTIFACTS-EVIDENCE ArtifactStore and Library commands. One format adapter implements read, edit, save and export. Unsupported operations return a typed unsupported error rather than a silent no-op.

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

- Format choice is blocked on B-21.
- Evidence verifier beyond hash of the chosen format is out of this slice.

Each blocker has the owners named above. Resolution gate: owner decision recorded in this SPEC before status leaves DRAFT. Implementation tasks: 007-T01, 007-T02, 007-T03, 007-T04.

## 7a. Owner facts incorporated 2026-11-01

Read from ARTIFACTS-EVIDENCE, PRESENTATION-RUNTIME, and INTERACTIVE-ARTIFACT-RUNTIME.

- An Artifact moves only TRANSIENT to SAVED by explicit promotion and SAVED to ARCHIVED by archive. Publishing to an ARCHIVED Artifact fails with ARTIFACT_ARCHIVED. Concurrent publication fails with STALE_VERSION. listLibrary returns only SAVED artifacts.
- The same Idempotency-Key and payload returns the recorded result. The same key with a changed payload returns 409 CONFLICT.
- The local text-append route accepts only managed text/plain up to 1 MiB and pins If-Match, content version, Resource aggregate version, and exact parent ResourceRevision. It rejects archived Workspaces, archived Artifacts, external-resource content, and HTML.
- Desktop Save As operates on one MANAGED_BLOB version up to the 10 MiB IPC bound. Success creates no Artifact or Resource revision and no domain event. Destination bytes do not return through WebView IPC.
- Evidence level is REPORTED, OBSERVED, or VERIFIED. REPORTED cannot satisfy an OBSERVED or VERIFIED criterion. The Runtime append path admits only REPORTED Evidence.
- An interactive run is volatile: PREVIEW_ONLY, STARTING, RUNNING, STOPPING, STOPPED, FAILED, REVOKED, or INTERRUPTED. It does not increment Artifact.version, does not start automatically, and does not run an ARCHIVED Artifact. Unproved quiescence is INTERRUPTED, not STOPPED.
- RichPresentationAvailability other than AVAILABLE still renders the committed semantic message. Presentation payloads forbid raw HTML, scriptable SVG, inline handlers, CSS, and arbitrary JavaScript. PresentationStatus COMPLETE does not mark a Task complete.
- Cite getArtifact, getArtifactTextEditHead, appendArtifactTextVersion, promoteArtifact, archiveArtifact, downloadArtifactVersion, listLibrary.



## 19. Requirement traceability

| Requirement | Spec | Canonical contract | Task | Acceptance |
|---|---|---|---|---|
| AC-001 | EARS acceptance | canonical owner docs cited in this SPEC | 007-T01 | AC-001 |
| AC-002 | EARS acceptance | canonical owner docs cited in this SPEC | 007-T01 | AC-002 |
| AC-003 | EARS acceptance | canonical owner docs cited in this SPEC | 007-T03 | AC-003 |
| AC-004 | EARS acceptance | canonical owner docs cited in this SPEC | 007-T04 | AC-004 |
| AC-005 | EARS acceptance | canonical owner docs cited in this SPEC | 007-T03 | AC-005 |
| AC-006 | EARS acceptance | canonical owner docs cited in this SPEC | 007-T02 | AC-006 |
