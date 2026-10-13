# V1 Agent-Operated Memory, Retrieval and Fast Path LLD

Status: normative TARGET, not implemented evidence. Domain authority: LEARNING.md, CONTEXT.md, RESPONSIBILITIES.md, TRUST.md and COWORKERS-TARGET.md. First-party Core owns eligibility and source provenance, not vector indexing or a hidden memory AI planner. Semantic search and indexing belong to qualified provider capabilities, optionally through LiteSPM.

## Scope and non-negotiable compatibility

There are three memories that must NOT be merged invisibly:
1. Native agent memory/instruction files and private native sessions. Native harness owns these. LiteCowork never writes them without an expressly qualified versioned sync contract with preview/revert, and cannot force when the agent performs its own recall.
2. Coworker, Workspace and Personal ContextDocuments, approved MemoryRecords, user corrections, source citations and immutable revisions. Core owns policy, provenance, deletion/revocation and authorizations; specialized extraction/retrieval is a provider.
3. Transient TaskPacket, ResumePacket, active ConversationTurn context and ContextUsed receipts. They are scoped to a real Turn/Task, not new permanent memories.

The agent chooses whether and when to invoke a qualified memory/search tool. LiteCowork can expose memory.search, memory.fetch, memory.propose and owner-authorized memory.list operations through the existing native or LiteSPM capability mechanism ONLY when the AgentModule supports them. memory.forget must be owner-authenticated and versioned, and is not a model-controlled destructive tool. Do not rewrite original native files or swap tool names. If an agent does not support this surface, show unavailable, not fake context injection.

Optional automatic context prefetch can occur only when explicit accepted TaskSpec/Workspace/Coworker policy and grants allow it. Use current explicit user instructions and TaskSpec first; memory evidence is lower-trust data and cannot override user instructions, authorization, grants or tool policy. Private memory never crosses Coworker boundaries by default. Temporary conversations never become extraction sources.

## Call/data path

~~~text
Qualified native agent memory/search request (or explicit TaskPacket prefetch)
  -> authenticated Operator/LiteSPM capability boundary
  -> current Workspace/Coworker/Task scope and source-grant check
  -> policy+revocation epoch gate
  -> versioned local metadata/cache hit?
       HIT: exact owner/revision/TTL + final revocation check
       MISS: qualified PersonalContextProvider or LiteSPM retrieval
         -> bounded index/search + optional rerank
         -> source refs/digest/eligibility/secret redaction
  -> final authorization check against live Resource revision and grant epoch
  -> ContextUsed receipt (what was supplied, never what model internally believed)
  -> bounded native tool result or explicitly authorized context attachment
~~~

No unsolicited global prompt injection or automatic re-summarization of every message. Save learned candidates only at meaningful committed Turn or verified Task checkpoints; candidate extractors cannot mutate TaskSpecs or publish durable memory without normal LEARNING eligibility policy.

## Actual latency target, not marketing

The in-process scope/policy/metadata/cache-hit logic should be profiled toward <=1ms p95 on warm hardware when possible. A true whole-operation sub-millisecond guarantee is NOT possible for IPC, disk, embeddings, remote provider, index search, reranking, or native process startup. Measure with a monotonic clock: native-tool request admission; IPC; policy; local cache; source/index provider wait; ranking; fetch; redaction; context packing; attachment delivery. Report p50/p95/p99 and real hit percentage by bounded class. Do not combine cached policy hit latency and uncached full retrieval into one alleged sub-ms metric.

A cold miss obeys the accepted Task and agent's bounded deadline, size and token budget. Unsupported async native tools receive bounded TIMEOUT/UNAVAILABLE rather than a fake pending handle; use deferred continuation only when native contract supports it. No retrieval on UI keystrokes, no network on user selection hot path, no unbounded fanout. Retrieval can be coalesced and prewarmed for explicit authorized context, never as a means of bypassing source consent.

## Typed targets and function contracts (proposed)

~~~ts
type MemorySearchRequest = {
 workspaceId:string; coworkerId?:string; taskId?:string; turnId?:string;
 purpose:"CONVERSATION"|"TASK_PLANNING"|"ATTEMPT"|"USER_INSPECTION";
 query:string; maxResults:number; maxTokens:number; deadlineMs:number;
 allowedSourceKinds:string[]; expectedPolicyEpoch:number;
};
type MemoryHit = {
 memoryRef:string; sourceResourceId:string; sourceRevision:string;
 sourceDigest:string; ownerScope:string; excerpt?:string;
 status:"ACTIVE"|"STALE"|"REVOKED"|"NEEDS_REVIEW";
 score?:number; lastConfirmedAt?:string; sourceType:string;
};
type MemorySearchReceipt = {
 status:"FOUND"|"EMPTY"|"UNAVAILABLE"|"TIMEOUT"|"DENIED"|"STALE";
 hits:MemoryHit[]; indexVersion?:string; policyEpoch:number;
 cache:"HIT"|"MISS"|"BYPASS"; totalMs:number; policyMs:number;
 providerMs?:number; returnedTokens:number; truncated:boolean;
};
~~~

Authenticated principal is derived from transport, never accepted from request payload. Result IDs do not grant fetch access: memory.fetch rechecks the exact ResourceVersion and owner scope. Operator candidate routes require OpenAPI/Events/Storage promotion; the above is not an existing endpoint.

| Function | Precondition | Output and error/race semantics |
|---|---|---|
| resolveAgentMemoryTools(binding, descriptor, scope) | qualified AgentModule/native surface | visible supported tool descriptors or UNAVAILABLE |
| checkMemoryAccess(principal, scope, policyEpoch) | authenticated actor and current grants | ALLOW/DENY/STALE, never infer from display |
| searchScopedMemory(request) | bounded owner-scope query | sourced hits/EMPTY/UNAVAILABLE; no source leak |
| fetchMemory(memoryRef, revision, purpose) | fresh revision, exact grant | bounded content/REVOKED; final status check |
| getScopedMemoryCache(key, grantEpoch) | scope/revision TTL and capacity | HIT/MISS with revocation recheck |
| invalidateMemoryCache(scope, changedRevision) | Resource/grant/source change | cache epoch bumped; stale inflight answers discarded |
| prepareMemoryAttachment(taskOrTurn, hits, nativeDescriptor) | eligible low-trust context | bounded context receipt or incompatibility |
| proposeLearningFromCheckpoint(verifiedSource, policy) | committed Turn / verified Task | MemoryCandidate only; no unchecked auto-commit |
| observeMemoryLatency(operation, stages) | monotonic, redacted timing | safe histogram+trace without memory contents |
| revokeMemorySource(owner, exactVersion) | authenticated owner, approved purge plan | tombstone/revocation and receipt saga |

## Cache invariants

Cache key contains authenticated owner + Workspace/Coworker scope + Task/purpose + permission digest/epoch + source revision set + index provider/version + query/filter digest + result/token bounds + redaction policy. TTL and LRU limits are explicit. Sensitive cache data stays ephemeral or encrypted in registered stores and never crosses owners. Source edits/revocation/deletion, grant changes, index rebuild, AgentBinding change, workspace switch and task-policy revision invalidate affected keys.

Old inflight provider search results are discarded if they fail the final authorization check. A revoked source may already have been sent to an external native session; stop use at the next safe boundary and avoid resuming that session with stale attachments. Do not pretend to recall bytes already sent. Purge receipts/tombstones follow CONTEXT and STORAGE.

## Visual UX based on screenshot, adapted for LiteCowork

Memory page has search, Add Memory, clear empty state, but must show My Context / Coworker / Workspace / Pending Review / Native (read-only when qualified). Each memory item exposes source, exact revision, scope, inferred-vs-confirmed, visibility, last confirmed timestamp, conflict/revocation status, corrections and inspectable ContextUsed references. Add/Edit/Correct/Forget are owner-versioned commands, not direct modification of agent files. Currently the desktop Resource upload UI supports only Workspace-owned WORKSPACE_NOTES; Coworker/Personal Add Memory is a TARGET and must be disabled with an explicit explanation until the typed owner-scoped ResourceService upload, ContextDocumentCreateMetadata and authorization path is implemented. Manual Add Memory produces a user-authored ContextDocument, never a fabricated LearningCandidate or auto-learning receipt. Agent tool activity may show Search/Fetch with observed latency/refs; it must not reveal protected memory in logs or claim the agent definitely used a result.

## Tests and failures

- Native agent never calls memory tool: no unsolicited lookup unless the owner explicitly enabled TaskPacket prefetch.
- Warm cache metadata/provenance operation is timed separately from IPC and provider query; never fake sub-ms provider recall.
- Cross-Workspace hot cache hit is denied even when query text is identical.
- Revocation while provider search is in flight causes final gate rejection and stale hit invalidation.
- Provider down != no memory: show UNAVAILABLE versus EMPTY.
- A MemoryCandidate from a third-party email/web page cannot directly commit as user preference.
- Two confirmed conflicting memories require review; no last-writer-wins.
- Deleted ContextDocument cannot be fetched from old result refs; external purge completion remains receipt-backed.
- Native harness memory file updates do not produce silent LiteCowork edits.
- Long retrieval returns bounded timeout or qualified continuation; Task may proceed only if policy permits.
- Temporary chat is not indexed nor automatically learned; no cross-agent private memory leaks.
- ContextUsed shows actual sources attached, not model's internal reasoning or unobserved accesses.

Canonical source documents remain LEARNING, CONTEXT and TRUST. Do not add a second planning agent for memory.
