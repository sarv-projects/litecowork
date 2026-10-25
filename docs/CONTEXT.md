# Workspace and Agent Context

## Durable Workspace instructions

A Workspace may have versioned user-authored instructions. These are explicit operating
rules and shared context, not hidden cognitive memory.

```text
WorkspaceInstructionRevision {
  workspace_id
  revision
  parent_revisions[]
  content_ref
  content_digest
  authored_by
  created_at
}
```

Workspace points to its current instruction revision. Revisions are immutable and
Hub-authoritative: an edit uses `If-Match` and names the current revision as its parent;
a stale write returns `STALE_WORKSPACE_VERSION`. A deliberate merge may name multiple
existing parents. Offline writes remain pending intents until accepted by the Hub. A Task
pins the revision used when its TaskSpec is created; a revision change affects new Tasks
and an existing Task only after the user explicitly creates a new TaskSpecRevision that
selects the newer revision. Every TaskPacket and ResumePacket names the pinned revision.
Conversation sessions receive the current revision at session start, and a later change is
delivered only at a safe turn boundary with provenance.

Instructions cannot grant capabilities, approve Effects, loosen Workspace policy, or
override system/security rules. They are untrusted content for security purposes and are
limited to 64 KiB of UTF-8 text per revision, versioned, digest-checked, and redacted from
external telemetry. The referenced Resource revision is pinned, must belong to the same
Workspace, and must be its current head when a new instruction revision commits. The
immutable instruction pin remains valid if the Resource later advances.

## Context boundaries

LiteCowork persists durable product state and constructs bounded, explicit context
projections. It does not synchronize native agent private prompts, hidden reasoning,
private transcripts, or machine-local authentication state.

```text
Conversation context:
  selected Workspace instruction revision
  ordered visible ConversationMessages
  active-turn UserRequests and their immutable responses
  explicitly attached ResourceRefs
  selected recent conversation summary, if generated and labeled

TaskPacket:
  TaskSpecRevision and pinned WorkspaceInstructionRevision
  accepted PlanRevision and relevant Step
  selected input ResourceRefs and exact revisions
  decisions and results relevant to this Step
  required outputs and acceptance criteria
  authorized grants/secret lease references (never bytes)

ResumePacket:
  TaskSpecRevision and pinned WorkspaceInstructionRevision
  current PlanRevision and active/completed Step IDs
  important decisions, Artifact/Evidence refs, unresolved questions
  failed strategies, remaining criteria, open Invocation/Effect state
  capability locks and source Attempt/checkpoint refs
```

Context construction is progressive. A Resource search result is metadata, not an
implicit attachment. The desktop may explicitly request deterministic lexical search over
the revision-scoped encrypted local text index, or use the bounded
`ON_DEMAND_CONTENT` compatibility scan. Indexed snippets are untrusted display data,
derived from the exact pinned ResourceRevision, and are never silently added to an
AgentSession. The result still does not attach bytes. The Agent or user selects bounded,
pinned ResourceRefs; ResourceResolver then checks scope, revision, freshness, and
availability before content is exposed.
Conversation scope cannot read Task-only data without an explicit reference and policy
authorization.

## Agent context attachments

`ContextAttachment` identifies a typed source, revision/digest, purpose, and byte/token
budget hint. Agent Fabric records which attachments were sent to a session. A replacement
session receives a fresh projection built from current durable state rather than assuming
the old transcript can be recovered. Native AgentSession snapshots are optional
optimizations and cannot override TaskSpec, grants, approval, or lease state.

Workspace instructions, TaskPackets, ResumePackets, and summaries are data, not authority.
Prompt-injected content inside them cannot create a grant, SecretLease, Approval, or
state transition.

## Memory boundary

First-party durable context consists of Workspace instructions, Conversations, Tasks,
Artifacts, user settings, Connections, approved Automations, and Resource metadata. A
semantic profile, vector index, knowledge graph, or candidate extraction is a
qualified external capability. The Core owns eligibility policy, provenance,
revisions, durable context metadata and retrieval safeguards, but never a separate
model-driven planning loop. Saving reusable procedures follows the
user-reviewed SkillProposal flow in `CAPABILITY-FABRIC.md` and publishes through LiteSPM
only after approval.

## Personal context and user-editable documents

The optional `PersonalContextProvider` contract and its authorization boundary are in
[`RESPONSIBILITIES.md`](RESPONSIBILITIES.md). Core does not implement vector storage,
embedding, ranking, or autonomous extraction. Provider-returned context is untrusted
content and carries source references, scope, version/digest where available, and retrieval
time. A provider cannot change a TaskSpec, policy, grant, Approval, or SecretLease.

User-authored profile, Coworker, Workspace, and Goal notes are versioned Resources with
`context_document: ContextDocumentMetadata` identifying their kind and owner. They use
normal Resource identity, access, freshness, retention, and deletion behavior. Edits use
Resource revision concurrency; conflicting offline edits require rebase/merge and never
use last-writer-wins.

Context precedence is current user instruction and accepted TaskSpec, explicit current
attachments, Workspace instructions, linked Goal context, Coworker instructions,
user-confirmed ContextDocuments, then retrieved historical context. A lower-priority
source cannot override a higher one. Material conflicts become a clarification or blocker.
Current implemented storage has no provider-generated memory proposal path. The
accepted target adds automatic, quiet, scoped memory proposals from authorized
conversation checkpoints and verified completed work. Proposed records, gating,
and the future implementation contract are in [Coworker target](COWORKERS-TARGET.md).
This is NOT permission to mine every connected account or a claim the learning
pipeline is currently operational. Until the corresponding verified implementation
exists, owner-authored ContextDocuments may be revised, revoked, or deleted through
ResourceService. Revocation removes a document from future context retrieval while
retaining its bytes. The current local deletion command accepts only Resources with local
managed content and fails closed before writing a tombstone if any exact revision has an
external/provider-backed location. For accepted local content, deletion blocks reads at
commit and purges content and derived indexes from Core-managed replicas; the tombstone and
revision digests remain so historical Task provenance does not disappear. A future,
qualified provider-backed deletion path may include registered owned replicas and remain
pending until their adapter confirms removal; the local command does not start that saga.

Resource edits use the authenticated owner route `POST /v1/resources/{resource_id}/revision-uploads`, an immutable parent
revision set, digest-verified chunk upload, and atomic commit. `If-Match` pins the current
Resource version when the upload session is created. A stale write returns
`RESOURCE_CONFLICT` without accepting bytes; an explicit resolution names every intended
parent revision. An unknown, cross-Resource, or duplicated parent returns
`RESOURCE_REVISION_PARENT_MISMATCH`; a valid but stale parent-head set remains
`RESOURCE_CONFLICT`. The owner upload is bounded to 100 MiB and requires the complete
current head set. A successful append emits `resource.revision.created.v1`, never
`resource.created`; Resource head, ancestry, managed location, derived index/invalidation,
upload state, event/snapshot, and idempotency receipt commit atomically. The head advances
atomically and never uses last-writer-wins.

`GET /resources/{resourceId}` remains available for tombstone metadata, but any content
resolution checks `ContextDocumentStatus` at the last Core authorization boundary. A
revocation fences new reads and TaskPacket/context attachment. Storage checks status when
admitting a content read, before fetching or decrypting Resource bytes. A read admitted
while status is `ACTIVE` may finish if a concurrent status change commits after admission;
the read admission is not a revocation barrier for already-started work. Any operation
that publishes derived state from those bytes must recheck status in its final transaction
and discard the result if the document is no longer `ACTIVE`. `REVOKED` retains bytes but
blocks new reads until restored; `DELETION_PENDING` and `DELETED` also block reads while
purge proceeds or after completion. Owner metadata remains available so status and the
appropriate restore/deletion outcome can be shown. Local ContextDocument purge does not
automatically stop or replace an active native session that already received the document.
Native history already observed by an external agent cannot be recalled. Any future
session-stop/replacement behavior requires a separately qualified session-lifecycle path;
it is not implied by the Resource tombstone. Existing Task/Evidence provenance retains the
Resource ID, revision IDs, and digests without restoring revoked/deleted bytes.

Deletion is a recoverable purge saga. The local V1 path uses indexed, bounded manifest admission:
`MAX_CONTEXT_PURGE_TARGETS=4096`, `MAX_CONTEXT_PURGE_REFERENCES=16384`, and
`MAX_CONTEXT_PURGE_MANIFEST_UTF8_BYTES=4194304` (4 MiB). The preflight reads at most each
limit plus one item to detect overflow. If any limit is exceeded, the request returns
`PURGE_MANIFEST_LIMIT_EXCEEDED` before changing status or deleting any bytes; the owner may
retry after reducing retained source revisions through normal Resource lifecycle. The sealed
manifest is processed in slices of at most 64 targets, with per-target idempotent receipts.
These constants are part of the API, event and storage contract.

Before `ACTIVE|REVOKED -> DELETION_PENDING`,
ResourceService seals an immutable `ContextDocumentPurgePlan` over the exact revision set
and each registered Core-owned blob/index replica selected by the applicable deletion
path. The current local command fails closed before the tombstone if any exact revision
uses an external/provider-backed location; its plan contains only local managed targets.
It also groups accepted `ResourceUploadChunk` objects and outstanding durable blob
reservations from linked revision-upload sessions in `OPEN` or `CONTENT_RECEIVED` as
`UPLOAD_CHUNK` targets by temporary blob digest, retaining each exact `(upload_id,
chunk_index)` in the sorted `target_upload_chunk_refs` array; their `target_revision_ids` is
empty. In the same SQLite transaction, each linked session becomes
`EXPIRED` with `reason_code=CONTEXT_DOCUMENT_DELETION`, while its session, chunk, and request
ledger metadata remains as non-resumable history. The plan, pending receipts, and durable
local tombstone commit atomically; a verified empty plan is explicit. The tombstone
immediately blocks content reads. A local target is acknowledged only after exact physical
removal and durable local reference/fence cleanup. A future qualified provider path may add
registered owned replicas; a Runtime receipt there is accepted only for the current Runtime
incarnation and exact plan target. `DELETED` is committed only when the receipt set exactly
equals the sealed target set and every Resource, derived-index, and upload-chunk target is
acknowledged; an empty plan is admitted only after a SQL preflight verifies that the Resource
has no local or external locations, ResourceIndex rows, registered Resource objects, or
linked upload-chunk objects. Since completed Resource-content GC removes its registry row
while immutable revisions retain their digests, purge also reads each such exact revision
`BlobRef` outside the SQL transaction while holding its digest lock. Only BlobStore
`NotFound` proves the bytes are absent; a present object or any other read failure fails
closed. The admission transaction rechecks the SQL inventory before committing. Empty-plan
completion repeats the fresh SQL checks and exact reads before setting `DELETED`. Retryable
purge failures keep the tombstone
pending and never restore content. Receipts contain no bytes, locators, credentials, or
provider continuation handles and remain available for recovery and backup audit. Desktop
V1 is `LOCAL_ONLY`; the planned E13-S01 backup contract must preserve the tombstone, exact
plan, receipt states, and non-resumable upload ledger metadata, but backup is not yet claimed
as implemented. Cross-Runtime replication is post-V1: current event payloads do not
reconstruct an unacknowledged plan, and future Mesh transfer must carry the exact plan and
per-target receipt state or an equivalent reconstructible record. External semantic indexes
are purge targets only when their provider adapter is registered as an owned replica;
arbitrary retrieval providers cannot extend or override Resource deletion truth.

For a human-facing Conversation response, optional Host Guidance is appended only as
zero-authority presentation advice after the current Conversation/task context is assembled.
It is not user-authored context, Workspace instruction, Task planning input, or worker
execution policy. It is omitted from planning and Attempt sessions by default. The exact
HostSkill and instruction digest used for a RichPresentation is retained as provenance;
native agent configuration remains untouched.

The Operator may offer a read-only “Context used” view for a Conversation turn or Task.
It is derived from the exact ContextAttachments and provider retrieval receipts actually
resolved for that scope, not a guess about what the Agent considered. It exposes source,
scope, revision/digest and freshness where available, with links to authorized source
views. A provider that cannot return source provenance is identified as such. This view
does not create a Core-owned semantic-memory store or a memory proposal path. Rendering and
revocation behavior are specified in [`PRESENTATION-RUNTIME.md`](PRESENTATION-RUNTIME.md).

## Scoped automatic memory target and revocation test obligations

On meaningful committed Conversation-turn and verified Task-outcome checkpoints, an eligible agent-backed extractor may produce bounded memory candidates; no independent Core model planner is introduced. Deterministic gates validate consent eligibility, exact source revision, Workspace/Coworker/private scope, third-party privacy and secret classifications, provenance, deduplication, contradiction, age and budget. Only approved private eligibility permits quiet automatic commit. Connected sources are searchable KNOWLEDGE by default, not automatically mined into permanent memory. Memory corrections create immutable superseding revisions; stale or conflicting claims remain reviewable and do not overwrite evidence. Temporary chats never queue extraction, including deferred summaries. Shared Workspace/personal learning requires separately authorized policy. ContextUsed disclosures show only actual attached or retrieved sources. Revocation immediately blocks newly admitted retrieval and invalidates allowed context caching. It does not stop or replace an already-active external native session, recall bytes already delivered, or rewrite that session's transcript. New turns and Task attempts recheck the current grant and source state. Provider deletion completion is truthful and receipt-backed. Extraction, metadata and read API are proposed—not current product capability.


## Learning versus context — finalized 2026-10-10

Context retrieval and Coworker learning are separate pipelines. Context answers "what
authorized material should this turn see?" Learning answers "what durable Coworker state,
if any, should change after an eligible correction/outcome?"

[Learning](LEARNING.md) keeps memory Resource/ContextDocument-backed and source-provenanced.
Third-party email/web/chat/document content may be retrieved for a Task but is ineligible
for automatic memory commit by default. A context source cannot create an instruction,
Skill, Routine, Automation, Grant or Connection scope merely by containing imperative text.

Revoking context access fences future retrieval. Revoking a learned MemoryRecord separately
fences future memory retrieval; neither operation pretends to erase text already sent to a
native provider in an earlier turn.

## Scoped agent memory retrieval and fast-path qualification

[V1 Agent Memory Fast Path LLD](V1-AGENT-MEMORY-FAST-PATH-LLD.md) is subordinate to the ContextDocument revision/grant/revocation/deletion truth above. Qualified AgentModules may expose memory search/fetch under LiteSPM/native protocol, but Core does not take over native memory search decisions. Cache keys must include principal, Workspace, Coworker, current source and grant epochs; each final fetch/attachment rechecks revocation, including on a hot cache hit. The timing budget applies to local policy/cache operations, never an uncached semantic search or model call.
