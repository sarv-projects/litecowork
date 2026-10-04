# Artifacts, Effects, Evidence and Verification

## Artifact ownership

Providers create bytes/content; LiteCowork owns durable identity, immutable versions, provenance and presentation references.

## ArtifactStore

```text
interface ArtifactStore {
  create(CreateArtifactRequest) -> Artifact
  add_version(AddArtifactVersionRequest) -> ArtifactVersion
  get(ArtifactId) -> Artifact
  get_version(ArtifactId, version) -> ArtifactVersion
  open_blob(BlobRef) -> BlobStream
  promote_to_library(PromoteArtifactRequest) -> Artifact
  archive(ArchiveArtifactRequest) -> Artifact
  list(ArtifactQuery) -> Page<Artifact>
}
```

ArtifactVersion is immutable after publication. Every Artifact has one stable Resource of kind `ARTIFACT`; each ArtifactVersion maps to exactly one immutable ResourceRevision of that Resource. `create` requires already committed content and atomically creates the Artifact, Artifact Resource/revision, version 1, and their creation events (`resource.created.v1`, `resource.revision.observed.v1`, `artifact.created.v1`, and `artifact.version.created.v1`). `add_version` appends both an ArtifactVersion and corresponding ResourceRevision under the caller's expected Artifact aggregate version and emits both version/revision events; it cannot accept a blob until BlobStore has verified and committed its digest. During creation, establish the Resource's current revision before inserting its ArtifactVersion; the database checks that the initial Artifact version and Resource head agree. During later publication, advance the Resource head before the Artifact current-version pointer, whose update is rejected unless it names the matching ResourceRevision. Both operations commit all rows, pointers, and events in one transaction.

```text
CreateArtifactRequest {
  workspace_id: WorkspaceId
  task_id: TaskId?
  kind: string
  display_name: string
  content: ArtifactContent
  input_refs: PinnedResourceRef[]
  created_by_attempt: AttemptId?
  provenance: ProvenanceRecord
  verification_refs: EvidenceId[]
}

AddArtifactVersionRequest {
  artifact_id: ArtifactId
  expected_version: u64
  content: ArtifactContent
  input_refs: PinnedResourceRef[]
  created_by_attempt: AttemptId?
  provenance: ProvenanceRecord
  verification_refs: EvidenceId[]
}

PromoteArtifactRequest {
  artifact_id: ArtifactId
  expected_version: u64
}

ArchiveArtifactRequest {
  artifact_id: ArtifactId
  expected_version: u64
}

ArtifactQuery {
  library_status?: ArtifactLibraryStatus
  task_id?: TaskId
  cursor?: string
  limit: u32
}
```

Draft editors may use provider-specific temporary state; publishing creates a new immutable version and ResourceRevision.
`MANAGED_BLOB` content must be fully committed and digest-verified before its version/event
is visible. `EXTERNAL_RESOURCE` content pins a stable ResourceRef and provider revision;
it records an observed digest only when supplied by the provider and does not copy bytes
into BlobStore. Artifact IDs are stable; versions are monotonically increasing per Artifact.
Adding a version requires the expected Artifact aggregate version. In one transaction, the store assigns the next integer version, appends the immutable ArtifactVersion and matching ResourceRevision, advances Artifact.current_version and Resource.current_revision_id, increments their aggregate versions, and appends both events. The current Artifact and Resource pointers must identify the same version. Concurrent publication loses with STALE_VERSION; it cannot overwrite or silently branch. The caller must re-read and explicitly rebase or publish a separate Artifact. Publishing to an ARCHIVED Artifact fails with ARTIFACT_ARCHIVED. Library promotion/archive changes library_status and Artifact.version, not the content version or Resource head. A directory/tree is represented as a manifest of child refs plus content digests, not as an unbounded local path.

An Artifact may be generated, uploaded, imported, or linked. A linked Artifact retains
its external provider and revision; it does not imply that bytes were replicated. A
stale or unavailable linked revision is labeled as such. Publishing a new version never
overwrites an earlier one.

`listLibrary` returns only SAVED artifacts. `list(ArtifactQuery)` returns authorized artifacts and supports the Archived Library filter.

An Artifact follows TRANSIENT -> SAVED -> ARCHIVED. Only explicit Library promotion moves TRANSIENT to SAVED; archive moves SAVED to ARCHIVED. Both commands use If-Match/expected_version and increment the Artifact aggregate version on transition. Repeating an already-applied command with the same Idempotency-Key returns the recorded result; archiving an already archived Artifact with the current expected Artifact version returns its current representation without another transition event; a stale expected version still returns STALE_VERSION. Archived artifacts retain immutable versions and authorized direct reads, but disappear from the default Library projection. ArtifactStore owns these transitions and emits the matching event.

## Provenance

`ProvenanceRecord` and `ProvenanceTransformation` are shared value types defined
canonically in `SCHEMAS.md`. ArtifactStore requires `ArtifactVersion.input_refs` to equal
the distinct refs in `provenance.source_inputs` and every transformation's `inputs`; an
external ArtifactContent ref is included too. This is the exact dependency set used to
build reverse invalidation edges. Provenance retains each consumed-byte digest and the
provider/capability, transformation, and tool-report references that explain how content
was produced.

## Effect service

```text
interface EffectService {
  propose(ProposeEffectRequest) -> Effect
  mark_started(EffectId, fence) -> Effect
  acknowledge(EffectId, ResultRef?) -> Effect
  observe(EffectId, Observation) -> Effect
  verify(EffectId, EvidenceId) -> Effect
  begin_reconciliation(EffectId, ReconciliationRequest) -> Effect
  authorize_retry(EffectId, ReconciliationEvidence) -> Effect
  fail(EffectId, FailureRecord) -> Effect
  mark_ambiguous(EffectId, AmbiguityRecord) -> Effect
}
```

A LiteCowork-mediated consequential call must have Effect(PROPOSED) persisted before
external mutation starts. Grant, approval, active Attempt, and current lease/fencing
authorization are checked at dispatch. The runtime-private fencing credential is supplied
only to the enforcing provider; durable Effect/lease records contain no raw credential.
The Effect binds the request digest, capability/operation, target,
Attempt, and stable idempotency key when the provider supports one. Read-only calls may
omit an Effect unless audit policy requires it.

An agent's direct native tool call is outside this service unless its adapter/provider
routes it through an equivalent enforcing boundary. LiteCowork records such a call only
as reported evidence or an independently observed Effect; it does not fabricate an
Effect receipt or claim it was fenced.

## Reconciler

```text
interface EffectReconciler {
  supports(Effect) -> bool
  reconcile(Effect) -> ReconciliationResult
}

ReconciliationResult =
  CONFIRMED_OCCURRED(observation)
  CONFIRMED_NOT_OCCURRED
  STILL_AMBIGUOUS(reason)
  FAILED(reason)
```

Retry after ambiguity is permitted only if not-occurred is established or the operation is provably idempotent with same idempotency key.
Effect has a monotonically increasing `dispatch_ordinal`. The same logical retry reuses
the original idempotency key and records a new `effect.started` event/dispatch ordinal
only after reconciliation authorizes it. A changed operation or target requires a new
Effect.
Reconciliation has a bounded timeout/retry policy. If it remains ambiguous, Task
completion is blocked and a human decision may append a resolution with explicit
provenance; the historical ambiguous record remains visible.

## Evidence

Evidence levels:

```text
REPORTED  # agent/provider says it happened
OBSERVED  # LiteCowork independently observes state/result
VERIFIED  # required postcondition independently checked
```

Evidence is immutable and append-only.
Each Evidence item names its subject, producer, observation method, time, and optional
payload digest/ref. Verification evidence is additionally bound to one TaskSpecRevision,
criterion digest, verifier/version, exact revision-pinned input ResourceRefs, and the
digests of the bytes actually consumed. Agent output is untrusted data. `REPORTED` can support a progress
display but cannot satisfy an `OBSERVED` or `VERIFIED` criterion. A later independent
check appends a new record at its own level.

## Verifier

```text
interface Verifier {
  supports(criterion, subject) -> bool
  verify(VerificationRequest) -> VerificationResult
}
```

Selection order:
1. deterministic schema/format validator
2. file/hash/existence check
3. executable tests
4. provider/API reconciliation
5. render/visual comparison
6. structured postcondition evaluator
7. independent verifier agent
8. user approval

Verifier runs are read-only with respect to the target Task, Artifacts, Resources, and
external Effects; they receive bounded read grants only. Use semantic/LLM verification
only when deterministic verification cannot express the criterion.
Verifier output includes the exact criterion revision, pinned input refs and digests, verifier identity,
result, and Evidence refs. A verifier cannot alter the TaskSpec or source Artifact. A
user may accept an exception only through an explicit approval decision, recorded as
Evidence at the assurance level allowed by policy.

## VerificationRun

```text
VerificationRun {
  verification_run_id
  task_id
  criterion_id
  task_spec_revision
  criterion_digest
  verifier_kind
  verifier_version
  subject_refs[]
  inputs: ResourceInput[]
  status: PENDING | RUNNING | PASSED | FAILED | INCONCLUSIVE
  evidence_refs[]
  started_at?
  completed_at?
}
```

`inputs` pairs each exact pinned Resource revision with the optional digest of bytes
presented to the verifier. This consumer-observed digest is distinct from an optional
provider-observed digest on `ResourceRevision`; when both exist they must match, or the
run fails with `INTEGRITY_FAILURE` and cannot produce passing Evidence.

A criterion ID is not a stable meaning by itself: changing criterion text, TaskSpec
revision, relevant source Resource revision/observed digest, or verifier semantics requires a new run. A prior
PASSED run cannot satisfy a changed criterion or changed inputs. Dependency invalidation
marks downstream verification projections stale without modifying immutable Evidence or
ArtifactVersion records.

## Library

Library is a projection over durable resources. Saving/promoting is explicit. Linked external resources may appear in Library without copying bytes, but their external revision must be shown.

## Retention and integrity

ArtifactVersion content is content-addressed. A blob is eligible for garbage collection
only when no ArtifactVersion, required checkpoint, active transfer, or retention hold
references it. Evidence and Effect history is retained according to workspace policy and
cannot be removed while required for an open Task, dispute, or reconciliation. Blob
replication verifies per-chunk integrity when chunked and the final digest before marking
the target version available.
