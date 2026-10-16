# ContextDocument purge implementation plan

Story: E02-S03, local Resource/ContextDocument purge and recovery.

> **Migration correction:** The original implementation plan below predates the migration
> preservation review. Its initial placement of the ResourceIndex registry in v16 is
> superseded: v16 remains the Agent Registry migration, Resource content is v17, upload
> chunk recovery is v18, and ResourceIndex registry/backfill is v19. Current source and
> `STORAGE.md` define the authoritative migration sequence and v19 schema fingerprint.

## Contract frozen for this slice

- The owner starts deletion with a distinct idempotent command. The existing
  ACTIVE/REVOKED status route remains limited to those two reversible transitions.
- A purge plan is sealed in the same SQLite transaction as the
  `DELETION_PENDING` tombstone, pending receipts, status event, aggregate snapshot,
  and request replay receipt. Local managed content and derived-index objects are
  enumerated from their durable registries and exact Resource revision references.
  Linked in-progress revision uploads add both accepted chunk ledger rows and
  outstanding durable reservations; targets group by blob digest and retain each exact
  `(upload_id, chunk_index)` identity.
- Sort `target_revision_ids` by UTF-8 bytes. Sort targets by
  `(replica_ref UTF-8 bytes, replica_kind ASCII)`. Both Runtime identity fields are
  serialized as `null` for local replicas. Compute `manifest_digest` as
  `sha256:` plus lowercase SHA-256 of the RFC 8785 canonical JSON bytes for the
  ordered targets array.
- A target's `replica_ref` is its stable content-addressed digest, never a locator.
  An acknowledgement means the exact object was removed by the local BlobStore
  contract. If another Resource still references the same digest, the target stays
  pending until that reference is gone and the object is removed. The existing
  GC deletion fence remains authoritative during provider retries.
- Any external/provider-backed location makes the local purge command fail closed
  before committing a tombstone. Provider purge remains unavailable until a real
  adapter and target contract are qualified.
- The deletion-status GET reports the sealed target count and durable acknowledged
  count. `DELETED` is committed only after all targets are acknowledged; an empty
  plan is accepted only after the transaction verifies that no local or external
  replica is registered.

## Small vertical slices

1. Add the manifest/receipt digest rules and distinct request/status API contracts;
   cover schema and route fixtures before implementation.
2. Add storage-core purge types and domain commands while keeping status writes
   separate from irreversible purge admission.
3. Implement atomic SQLite plan/tombstone/request-replay admission, target-scoped
   reference removal, pending status reads, exact receipt admission, and finalization.
4. Connect the daemon's authenticated owner routes and bounded reconciliation pass
   to the existing fenced Resource and ResourceIndex collectors.
5. Add Tauri/Library delete confirmation and truthful pending, completed, and error
   projections after the Operator path is proven.
6. Run focused rollback, authorization, stale-version, replay, shared-reference,
   failure/restart, and event tests; then run the story and repository gates.

## Execution tasks

### Task 1: Freeze the local purge contract

**Files:** `docs/API.md`, `docs/FLOWS.md`, `docs/SCHEMAS.md`, `docs/DATA-MODEL.md`,
`docs/EVENTS.md`, `docs/WORLD-RESOURCES.md`, `docs/STORAGE.md`,
`docs/CONTEXT.md`, `docs/STATE-MACHINES.md`,
`docs/schemas/operator-api.openapi.yaml`,
`docs/schemas/domain-event.schema.json`,
`tests/test_context_document_purge_schema.py`.

**Acceptance:** A distinct POST deletion route accepts only `{}`, uses Workspace owner
authentication, `If-Match`, and `Idempotency-Key`; local purge rejects external locations
before writes; local target ordering and RFC 8785 digest rules are explicit; local event
Runtime fields are required JSON nulls; current deletion does not claim native-session
invalidation. The focused schema test passes.

### Task 2: Define domain and storage ports

**Files:** `crates/storage-core/src/lib.rs`, `crates/domain-workspace/src/lib.rs`.

**Acceptance:** `ResourceService` exposes an owner deletion command, owner-scoped status
lookup, and bounded reconciler. The storage port separates admission, exact receipt
acknowledgement, and verified empty-plan completion. Final target reconciliation can
commit both its acknowledgement event and the final DELETED status event atomically.

### Task 3: Implement the SQLite purge saga

**Files:** `crates/storage-sqlite/src/lib.rs`,
`crates/storage-sqlite/src/context_document_purge.rs`,
`crates/storage-sqlite/src/context_document_purge_tests.rs`.

**Interfaces:** Implement the `storage_core::ContextDocumentPurgeStore` port in SQLite
against the v1 purge-plan/receipt tables and the v17/v18/v19 object registries. Owner start must
commit the plan, pending receipts, Resource tombstone, aggregate snapshot, status event,
idempotency replay response, location unlink, and all ResourceIndex-row removals in one
transaction. Exact retry must replay before checking the now-advanced version. Target
acknowledgement must match the sealed plan and prove the appropriate registry row is
absent. On the last acknowledgement, commit the acknowledgement and DELETED status events
with the Resource version/snapshot. Empty plans require a fresh transaction proving no
local or provider-backed locations, ResourceIndex rows, or registered Resource objects
remain. Shared Workspace digests must remain pending until the existing fenced collector
removes them. Preserve Resource/revision identities and digests.

**Verification:** `cargo test --locked -p storage-sqlite context_document_purge_tests`.
The focused tests must cover deterministic manifest/JCS digest, owner denial, external
source fail-closed without writes, exact replay/stale version, transactional rollback,
shared-root wait, registry-absence acknowledgement, and verified-empty completion.

### Task 4: Serve authenticated Operator routes and recovery

**Files:** `apps/litecoworkd/src/operator.rs`,
`apps/litecoworkd/src/operator_ipc_tests.rs`,
`apps/litecoworkd/src/resource_content_gc_operator_tests.rs`.

**Acceptance:** Mount the documented POST and GET routes with the existing Workspace
owner, idempotency, and version checks. The bounded daemon recovery loop runs the purge
reconciler with unique valid event contexts and reports only truthful committed counts.
Route tests exercise denial, malformed body/headers, replay, status polling, failed and
retryable collection, and no secret or locator leakage.

### Task 5: Add the Library deletion control

**Files:** `apps/litecowork-ui/src/resources/ResourceRevisionEditor.tsx`, its owning
helpers/tests/styles, and only the generated or typed Operator client declarations needed
for the promoted route.

**Acceptance:** Owner confirmation explains permanent local deletion; ambiguous outcomes
reuse the exact idempotency key; successful admission shows DELETION_PENDING and polls to
DELETED; transport/provider errors remain explicit. Controls and status are keyboard
accessible and do not claim that an already-fed native session was invalidated.

### Task 6: Record evidence and rerun gates

**Files:** `implementation/reviews/E02-S03.md`, `implementation/CURRENT-RUN.md`,
`implementation/backlog.json`, `implementation/coverage.csv` only if source-backed
coverage changes.

**Acceptance:** Record exact focused CODE and authenticated SYSTEM commands, fixture and
runtime versions, observed results, limitations, and the pending owner USER/packaged OS
gates. Do not change story status beyond the evidence proven. Run affected contract and
repository validators before checkpointing.

## Explicit evidence limits

Local CODE and authenticated route evidence can be completed in this environment.
Packaged desktop recovery, production key provider behavior, non-Linux qualification,
and owner USER acceptance remain separate gates and must stay pending until exercised.
