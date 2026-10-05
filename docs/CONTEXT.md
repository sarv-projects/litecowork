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
size-limited, versioned, digest-checked, and redacted from external telemetry.

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
implicit attachment. The Agent or user selects bounded references; ResourceResolver then
checks scope, pinned revision, freshness, and availability before content is exposed.
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
semantic profile, vector index, knowledge graph, long-term agent memory, or autonomous
memory rewrite is an external capability. Saving reusable procedures follows the
user-reviewed SkillProposal flow in `CAPABILITY-FABRIC.md` and publishes through LitePSM
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
Memory extraction produces a proposal for owner review; revocation is honored by Core and
is reported incomplete if an external provider cannot verify removal.
