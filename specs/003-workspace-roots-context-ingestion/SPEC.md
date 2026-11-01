# 003 - Workspace Roots and Context Ingestion

**ID:** 003
**Status:** DRAFT
**Owners:** Resource, Conversation, Storage, Security/Trust, Desktop UI
**Updated:** 2026-11-01
**Revision:** 0.1
**Target release:** V1 local desktop
**Dependencies:** root ARCHITECTURE.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md, docs/IMPLEMENTATION-MASTER-PLAN.md, and the owner documents named in RESEARCH.md. Feature 001 remains the Conversation send authority.

## 1. Problem

Chat without grounded files cannot cite local work. A one-time attachment is not a WorkspaceRoot. Search snippets must not become silent context. The machine must not be scanned wholesale.

Observable outcome: The owner attaches a folder as a persistent WorkspaceRoot, asks a grounded question with citations from exact revisions, and revokes the root so later retrieval is fenced.

## 2. Goals

- Create, pause, resume and revoke WorkspaceRoots with overlap rejection and private locators.
- Ingest bounded Resources with revision and digest identity.
- Expose Context Used only from actual ContextAttachments and retrieval receipts.
- After revoke, exclude that root from future retrieval while retaining historical provenance that does not re-expose bytes.

## 3. Non-goals

- Cloud replication of roots in V1 beyond the existing policy intersection.
- Treat pause as unavailable, or unavailable as revoked.
- Grant write, network or secret rights because a root exists.

## 4. Current evidence

This checkout is documentation-first. The behavior below is **TARGET**. Archived source and historical CODE notes are evidence of past work, not SERVED or QUALIFIED status on this tree. No owner has locked this package.

## 5. LOCKED and FLEXIBLE decisions

### LOCKED

- Root identity revalidation uses no-follow directory handles and exact OS identity (WORLD-RESOURCES).
- Canonical paths stay Runtime-local and never enter events or public errors.
- Overlapping non-revoked roots are rejected.
- Revoked roots cannot be reactivated.
- Root observation is not a capability grant.
- Context instructions cannot grant capabilities (CONTEXT.md).

### FLEXIBLE

- Module file layout and private helper names, provided the transactions and secret boundaries stay intact.
- Presentation copy, provided state distinctions remain visible.

### Open decisions

- Which content types are in the first qualified ingestion set.
- Watcher versus explicit rescan for the first GA OS.

### Blockers

- B-13: first qualified ingestion formats.
- B-14: watcher default versus manual rescan on the GA OS.

## 6. High-level design

UI picker returns a candidate path to the Operator. ResourceService commits root status, location availability and private bindings in one SQLite transaction. Context assembly reads only ACTIVE roots and pinned revisions. Conversation send remains feature 001 and only consumes an already built package.

Trust boundary: the desktop UI is untrusted; the authenticated Operator is the only command path; native credentials and OS handles stay outside public events; SQLite transactions do not perform network or process calls.

## 7. Low-level design

Follow WORLD-RESOURCES root creation, pause, resume and revoke transactions. ContextAttachments are explicit. Retrieval receipts record resource revision ids. Proposed indexing workers stay outside the admission transaction.

Ordering: authorize, compare expected version, commit the durable receipt, then perform any external call, then reconcile. Proposed payload fields are PROPOSED until the canonical owner document and machine schema change together.

## 8. Data

See [DATA-MODEL.md](DATA-MODEL.md). This feature does not create a second source of truth beside the domain owner.

## 9. Wiring

See [WIRING.yaml](WIRING.yaml). Requests enter through the Operator. External calls sit outside transactions.

## 10. Security

Owner authorization and Workspace version are checked on every mutation. Symlinks are not followed. Overlap uses component comparison, not string prefix. Private locators are absent from Operator error bodies. Revocation deletes private bindings.

## 11. Errors and recovery

UNAVAILABLE, PAUSED and REVOKED are distinct. Stale Workspace version conflicts without partial writes. Identity mismatch on resume leaves the root PAUSED. Lost keyring identity is a startup blocker and must not be reported as a successful scan.

Races use compare-and-swap or a single-writer occurrence identity. Timeout after an external effect reconciles. Duplicate retries return the original receipt when the idempotency key matches and fail when the payload conflicts.

## 12. Crash and restart

Reload the last committed owner records. Do not infer success from an intent. Reconcile in-flight external work before a new side effect. Cancellation fences late results.

## 13. Configuration and compatibility

Linux and Windows locator encodings differ and both fail closed on malformed input. macOS qualification is separate. Watcher is target-only until B-14 is decided.

OS qualification is per native desktop. WSL2 does not qualify Windows or Linux GUI behavior.

## 14. Observability

Log ids, versions, statuses and denial reasons. Do not log secret bytes, native handles, private locators or raw credentials. Audit commits include actor, workspace and receipt id.

## 15. EARS acceptance

- **AC-001:** WHEN the owner adds a folder root, THE SYSTEM SHALL commit root, location and private binding in one transaction or leave no partial current-incarnation binding.
- **AC-002:** IF the proposed root overlaps an existing non-revoked root, THEN THE SYSTEM SHALL reject it without revealing the other root's private path.
- **AC-003:** WHEN startup reopens a saved root, THE SYSTEM SHALL bind it only on exact OS identity match. IF that check fails for an ACTIVE root, THEN THE SYSTEM SHALL mark the root UNAVAILABLE. IF the root is PAUSED, THEN THE SYSTEM SHALL leave it PAUSED and mark the location UNAVAILABLE. IF the root is REVOKED, THEN THE SYSTEM SHALL NOT reopen it.
- **AC-004:** WHILE a root is PAUSED or UNAVAILABLE, THE SYSTEM SHALL exclude it from search and context assembly.
- **AC-005:** WHEN the owner revokes a root, THE SYSTEM SHALL fence future retrieval and SHALL NOT reactivate that root id.
- **AC-006:** WHEN a grounded answer is shown, THE SYSTEM SHALL list Context Used only from committed attachments and retrieval receipts, not from bare search snippets.
- **AC-007:** IF ingestion exceeds the configured byte or file bound, THEN THE SYSTEM SHALL fail that resource and continue unrelated roots.

## 16. Executable acceptance command

```bash
python3 .specify/scripts/validate_feature_package.py specs/003-workspace-roots-context-ingestion
```

Further gates after ACCEPTED/LOCKED are the cargo filters named in TASKS.md. They are not evidence until run on a recovered source tree.

## 17. Out-of-bounds

- Do not perform mediated file writes (feature 005).
- Do not start browser or terminal sessions.
- Do not replicate secret files into events.

## 18. Assumptions and open questions

- Served historical root metadata on Linux is not current qualification of this checkout.
- Windows focused tests in the archive are historical CODE, not QUALIFIED packaged startup.

Each blocker has the owners named above. Resolution gate: owner decision recorded in this SPEC before status leaves DRAFT. Implementation tasks: 003-T01, 003-T02, 003-T03, 003-T04.

## 7a. Owner facts incorporated 2026-11-01

Read from WORLD-RESOURCES, CONTEXT, and STATE-MACHINES.

- An ACTIVE root is not an implicit attachment. Context assembly admits only explicitly selected pinned ResourceRefs.
- A metadata census commits FILE membership only, reads no content, and creates no ResourceRevision. Bounds: 1,000 directory entries, depth 32, 1 MiB of relative locator bytes, an 8-second deadline that stops traversal at 7 seconds, and two concurrent jobs. A partial scan does not tombstone unseen Resources.
- Resume of PAUSED requires an AVAILABLE location and the current incarnation READY or DEGRADED. An UNAVAILABLE root is restored only by identity revalidation, never by resume.
- Keyring identity loss blocks startup before the root transaction and does not mark roots UNAVAILABLE.
- Revocation commits REVOKED, deletes private bindings, and removes selected-root replication membership in one transaction. Task history and Artifacts remain under their own retention.
- Overlap rejects the same, ancestor, or descendant path of every ACTIVE, PAUSED, or UNAVAILABLE root. Only REVOKED leaves the check. SELECTED_FOLDERS stores root ids. A root policy cannot broaden Workspace replication.
- Workspace instruction revisions are at most 64 KiB UTF-8. A stale write returns STALE_WORKSPACE_VERSION.
- Content reads require ContextDocumentStatus ACTIVE. REVOKED, DELETION_PENDING, and DELETED block new reads. Revocation does not rewrite a transcript that already received the bytes.
- An unpinned ResourceRef in CONFLICTED returns RESOURCE_CONFLICT. A digest mismatch against the pinned content_digest returns INTEGRITY_FAILURE.
- Cite listWorkspaceRoots, pauseWorkspaceRoot, resumeWorkspaceRoot, revokeWorkspaceRoot, scanWorkspaceRoot, searchResources, groundedRetrieval. addWorkspaceRoot and updateWorkspaceRoot are both target-only and have no mounted handler. Events: workspace.root.created.v1, workspace.root.status.changed.v1.



## 19. Requirement traceability

| Requirement | Spec | Canonical contract | Task | Acceptance |
|---|---|---|---|---|
| AC-001 | EARS acceptance | canonical owner docs cited in this SPEC | 003-T01 | AC-001 |
| AC-002 | EARS acceptance | canonical owner docs cited in this SPEC | 003-T01 | AC-002 |
| AC-003 | EARS acceptance | canonical owner docs cited in this SPEC | 003-T02 | AC-003 |
| AC-004 | EARS acceptance | canonical owner docs cited in this SPEC | 003-T02 | AC-004 |
| AC-005 | EARS acceptance | canonical owner docs cited in this SPEC | 003-T02 | AC-005 |
| AC-006 | EARS acceptance | canonical owner docs cited in this SPEC | 003-T03 | AC-006 |
| AC-007 | EARS acceptance | canonical owner docs cited in this SPEC | 003-T03 | AC-007 |
