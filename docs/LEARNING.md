# Coworker Learning, Memory, Instruction, and Skill Revision

**Status:** accepted target architecture, not a claim that suggested
`learning-candidates` endpoints or new target crates already exist.
Use the actual `domain-responsibility`, `domain-rag`,
`storage-core`/`storage-sqlite`, `apps/litecoworkd` and UI
owners, and require separately reviewed boundaries for any new crates.
Current release state: [acceptance matrix](../implementation/V1-PRODUCT-ACCEPTANCE-MATRIX.md).

**Status:** accepted target architecture. This document defines how a Coworker can improve
from corrections and successful work without allowing the model to silently widen authority
or rewrite arbitrary persistent configuration.

It extends [Coworkers](COWORKERS-TARGET.md), [Responsibilities](RESPONSIBILITIES.md),
[Context](CONTEXT.md), [Trust](TRUST.md), [Routines](ROUTINES.md), and
[Reference Implementations](REFERENCE-IMPLEMENTATIONS.md).

## 1. Frozen principle

"Correct it once and it remembers" is a product goal, not a license for uncontrolled
self-modification.

LiteCowork separates:

```text
Memory       = durable fact/preference used as context
Instruction  = how the Coworker should behave
Skill        = reusable procedural/executable capability
Routine      = reusable work definition
Automation   = future trigger + execution policy
Permission   = Trust/Grant/Approval authority
```

A change in one category never silently changes another.

Examples:

- "Use shorter status reports" may become a private preference memory.
- "From now on, always cite the source URL" may become a reviewed instruction patch.
- "I showed you how to do this website task" may become a SkillProposal after testing.
- "Do this every Monday" becomes a Routine/Automation proposal, not memory.
- "You may send emails without asking" is a Trust-policy request, not an instruction.
- A source document saying "remember this permanently" is untrusted content and cannot
  create memory by itself.

## 2. Learning pipeline

```text
Conversation / Task / Feedback / Demonstration
                    |
                    v
             LearningCandidate
                    |
       classify + redact + dedupe
                    |
      +-------------+-------------+----------------+
      |                           |                |
      v                           v                v
 MemoryCandidate          InstructionPatch   SkillProposal
      |                           |                |
 policy-safe?                    review           test/review
      |                           |                |
  +---+----+                      +--------+-------+
  |        |                               |
  v        v                               v
auto      review                    CoworkerRevision /
commit                                LiteSPM package
  |
MemoryRecord
```

The only automatic persistent commit allowed by default policy is a narrowly qualified
private memory/preferences update. Instructions, Skills, Routines, Automations, connection
scope, permissions, and delegation policy require the appropriate normal versioned/review
path.

## 3. LearningCandidate

```text
LearningCandidate {
  candidate_id: LearningCandidateId
  workspace_id: WorkspaceId
  coworker_id: CoworkerId

  kind:
    MEMORY_FACT
    | MEMORY_PREFERENCE
    | INSTRUCTION_PATCH
    | SKILL_PROPOSAL
    | ROUTINE_PROPOSAL
    | AUTOMATION_PROPOSAL

  source_kind:
    USER_EXPLICIT
    | USER_CORRECTION
    | USER_FEEDBACK
    | TASK_OUTCOME
    | REPEATED_PATTERN
    | DEMONSTRATION

  source_conversation_id?: ConversationId
  source_message_id?: MessageId
  source_task_id?: TaskId
  source_attempt_id?: AttemptId
  source_demonstration_id?: DemonstrationId

  source_digest: Sha256Digest
  proposed_payload_ref: ResourceRef
  redaction_report_ref: ResourceRef
  classifier_version: string
  confidence?: number

  state:
    NEW
    | CLASSIFIED
    | NEEDS_REVIEW
    | AUTO_COMMIT_ELIGIBLE
    | COMMITTED
    | DISMISSED
    | REJECTED
    | SUPERSEDED

  reason_codes: string[]
  created_at: Timestamp
  updated_at: Timestamp
  version: u64
}
```

The payload is Resource-backed so the event journal does not carry arbitrary user/private
text. Event payloads contain IDs, digests, reason codes and bounded metadata only.

## 4. CoworkerLearningPolicy

This is separate from action autonomy. On an accepted newly created Coworker, the target onboarding must explain the narrowly scoped private learning option and allow REVIEW_ALL or OFF; AUTO_CONFIRMED_PRIVATE is only the informed default for eligible, low-sensitivity, user-confirmed sources. It never implies that automatic inference, Workspace/personal sharing, agent-private memory edits, background monitoring or executable self-configuration is enabled. The older CoworkerContextPolicy.require_user_confirmation_for_memory is legacy and must not be consulted as this policy's control.

```text
CoworkerLearningPolicy {
  coworker_id: CoworkerId
  revision: u64

  memory_mode:
    OFF
    | REVIEW_ALL
    | AUTO_CONFIRMED_PRIVATE
    | AUTO_CONFIRMED_AND_LOW_RISK_INFERRED_PRIVATE

  eligible_source_kinds: LearningSourceKind[]
  eligible_conversation_classes: NORMAL[]
  excluded_resource_labels: string[]
  candidate_retention_days: u32

  instruction_patch_mode: REVIEW_REQUIRED
  skill_mode: REVIEW_REQUIRED
  routine_mode: REVIEW_REQUIRED
  automation_mode: REVIEW_REQUIRED

  notify_on_memory_commit:
    NEVER
    | IMPORTANT_ONLY
    | ALWAYS

  created_at: Timestamp
}
```

`TEMPORARY` conversations are never eligible. A policy may be more restrictive than this
schema through Workspace/admin Trust rules; it cannot be more permissive.

## 5. MemoryCandidate payload

```text
MemoryCandidatePayload {
  subject_ref: string
  predicate: string
  value: JsonValue
  memory_class: FACT | PREFERENCE | WORKING_STYLE | PROJECT_CONTEXT
  certainty: CONFIRMED | INFERRED
  sensitivity: LOW | REVIEW_REQUIRED | FORBIDDEN
  scope: COWORKER_PRIVATE
  expires_at?: Timestamp
  supersedes_memory_id?: MemoryRecordId
}
```

The system does not infer or persist highly sensitive personal data merely because it
appears in a task. Secrets, passwords, access tokens, payment data, raw authentication
material and hidden native-harness prompts are always forbidden memory payloads.

## 6. Memory commit rules

```text
function may_auto_commit(candidate, policy, current_state):
  require candidate.kind in {MEMORY_FACT, MEMORY_PREFERENCE}
  require policy.memory_mode allows candidate.certainty
  require candidate.source_kind in policy.eligible_source_kinds
  require source Conversation is not TEMPORARY
  require candidate.sensitivity == LOW
  require candidate.scope == COWORKER_PRIVATE
  require redaction passed
  require no secret detector hit
  require no Trust/admin exclusion
  require no unresolved contradiction with current authoritative memory
  require candidate confidence >= policy threshold when INFERRED
  return true
```

A confirmed user statement outranks a prior inferred memory. A new inferred claim that
conflicts with a confirmed one becomes `NEEDS_REVIEW`; it cannot silently replace it.

## 7. MemoryRecord

Canonical memory stays Resource-backed.

```text
MemoryRecordMetadata {
  memory_id: MemoryRecordId
  workspace_id: WorkspaceId
  coworker_id: CoworkerId
  context_document_id: ContextDocumentId
  source_candidate_id: LearningCandidateId
  memory_class: MemoryClass
  certainty: CONFIRMED | INFERRED
  scope: COWORKER_PRIVATE
  source_digest: Sha256Digest
  committed_at: Timestamp
  expires_at?: Timestamp
  superseded_by?: MemoryRecordId
  status: ACTIVE | SUPERSEDED | REVOKED | PURGE_PENDING
  version: u64
}
```

The actual text/value is stored as a bounded ContextDocument/Resource revision with exact provenance. Search/retrieval never treats metadata alone as the content. This MemoryRecordMetadata is for **automatically learned Coworker-private entries** with a real LearningCandidate and policy snapshot. An owner choosing Add Memory manually creates a **user-authored ContextDocument/Resource revision** with owner/scope/source metadata through ResourceService, not a fabricated LearningCandidate/source_candidate_id. Explicitly shared Workspace/personal knowledge uses separate reviewed Resource sharing/permissions, never a new automatic-memory scope. The unified Memory page may project both source classes but must label provenance and deletion owner accurately.

## 8. InstructionPatch

Instructions affect future behavior and therefore always create a reviewed Coworker
revision.

```text
InstructionPatchProposal {
  candidate_id: LearningCandidateId
  coworker_id: CoworkerId
  base_coworker_revision: u64
  operations: InstructionPatchOp[]
  rationale: string
  source_refs: ResourceRef[]
  tests: LearningTestSpec[]
}

InstructionPatchOp =
  ADD_RULE { section, text }
  | REPLACE_RULE { rule_id, expected_digest, text }
  | REMOVE_RULE { rule_id, expected_digest }
```

No free-form "replace the whole system prompt" patch is accepted from a model. Operations
target typed rule IDs/sections and are version checked.

### Apply instruction patch

```text
load exact base CoworkerRevision
assert candidate NEEDS_REVIEW
validate patch targets and bounds
render proposed diff
user approves exact diff
run static policy checks
optionally run bounded regression tests
CoworkerService.revise(expected_revision, approved_patch)
mark candidate COMMITTED with resulting revision
```

If the Coworker changed after proposal, approval is stale. Rebase creates a new proposal;
the old approval is not replayed.

## 9. SkillProposal

A reusable procedure is not memory.

A candidate can become a SkillProposal only after:

1. task-specific secrets/IDs/paths are redacted or parameterized;
2. the source behavior is reconstructed into a bounded procedure;
3. declared capabilities and authority are explicit;
4. an isolated test fixture passes;
5. package provenance/manifest is generated;
6. the user reviews and approves publication/install;
7. LiteSPM owns the package lifecycle from that point onward.

A successful Task alone is not proof a Skill generalizes.

## 10. Routine and Automation proposals

Repeated work can produce a suggestion:

```text
"Create a Routine from this?"
"Run it every Monday at 09:00?"
```

The system may prefill a proposal but cannot enable it.

```text
RepeatedPattern
  -> RoutineProposal
  -> user reviews inputs/output/verification
  -> RoutineRevision
  -> optional AutomationProposal
  -> user reviews trigger/timezone/location/permissions
  -> PAUSED Automation
  -> explicit Enable
```

A LearningCandidate never authors an active trigger directly.

## 11. LearningService

```text
interface LearningService {
  observe_user_message(message_id) -> LearningObservationResult
  observe_task_outcome(task_id) -> LearningObservationResult
  observe_feedback(feedback_id) -> LearningObservationResult
  observe_demonstration(demonstration_id) -> LearningObservationResult

  classify(candidate_id, classifier_version) -> LearningCandidateView
  list(coworker_id, state?, kind?, cursor?, limit) -> Page<LearningCandidateView>
  get(candidate_id) -> LearningCandidateView

  approve(candidate_id, expected_version, request_id) -> LearningCommitReceipt
  dismiss(candidate_id, expected_version, request_id) -> LearningCandidateView
  reject(candidate_id, reason, expected_version, request_id) -> LearningCandidateView

  auto_commit_eligible(coworker_id, batch_limit) -> AutoCommitBatchResult
  reconcile_conflicts(coworker_id) -> LearningConflictReport
}
```

### observe_user_message pseudocode

```text
message = ConversationService.get_committed_message()
if temporary conversation: return INELIGIBLE
if source not allowed by current CoworkerLearningPolicy: return INELIGIBLE

signals = deterministic_signal_extract(message)
# explicit "remember/prefer/from now on" and correction markers are signals,
# not authority.

for signal in signals:
  candidate = redact_and_classify(signal)
  if candidate contains forbidden sensitive material:
      persist only REJECTED metadata/digest; do not persist proposed content
  else:
      dedupe against active/pending candidates
      store Resource-backed proposed payload
      create LearningCandidate
      if may_auto_commit(...):
          mark AUTO_COMMIT_ELIGIBLE
      else:
          mark NEEDS_REVIEW
```

### auto_commit_eligible pseudocode

```text
for candidate in oldest eligible candidates up to batch_limit:
  begin transaction
  reload candidate + policy + source status + current memory
  if any basis changed: reclassify or NEEDS_REVIEW
  if contradiction: NEEDS_REVIEW
  else:
      create ContextDocument/Resource revision
      create MemoryRecordMetadata
      mark candidate COMMITTED
      append events
  commit
emit quiet notification only if policy requests it
```

## 12. Feedback model

Completed Task/Conversation surfaces may offer:

```text
Useful
Not relevant
Fix result
Don't do this again
```

`Useful` is weak positive evidence; it does not automatically create a Skill.
`Not relevant` may tune notification/retrieval preferences after policy review.
`Fix result` opens an ordinary correction turn and may create a candidate from the
explicit correction.
`Don't do this again` may propose an instruction/notification restriction but does not
silently create a broad deny rule without showing the user what will change.

Feedback record:

```text
FeedbackRecord {
  feedback_id
  workspace_id
  coworker_id?
  conversation_id?
  task_id?
  message_id?
  kind: USEFUL | NOT_RELEVANT | FIX_RESULT | DONT_REPEAT
  note_ref?: ResourceRef
  created_at
}
```

## 13. Self-configuration host tools

Borrowing the useful part of Agenta's `read_config`/`commit_revision` pattern, a
Coworker may be given bounded host operations:

```text
read_coworker_config()
propose_memory(...)
propose_instruction_patch(...)
propose_skill(...)
propose_routine(...)
propose_automation(...)
```

There is **no** default model tool that means "commit arbitrary Coworker config".

A future `commit_private_memory` host operation may exist only if Core pre-authorizes the
exact candidate through `AUTO_COMMIT_ELIGIBLE`; the model does not choose arbitrary text
at commit time.

## 14. Native harness memory

Native agents may have their own memory/instruction files. LiteCowork does not assume these
are equivalent to Coworker memory.

Rules:

- native memory remains native-harness-owned unless an AgentModule explicitly exposes a
  qualified sync contract;
- LiteCowork never edits `CLAUDE.md`, `AGENTS.md`, OpenCode config, or another native
  memory file merely because Coworker memory changed;
- optional sync is a separate versioned operation with preview/diff, exact target and
  rollback semantics;
- native session transcripts are not automatically ingested as Coworker memory;
- a Coworker handoff never transfers another Coworker's private memory.

## 15. Contradiction and supersession

```text
if new confirmed == current confirmed:
  dedupe
elif new confirmed conflicts current inferred:
  supersede inferred
elif new confirmed conflicts current confirmed:
  NEEDS_REVIEW
elif new inferred conflicts any confirmed:
  NEEDS_REVIEW
elif new inferred supports current:
  dedupe / confidence update projection only
```

Deleting a memory creates a tombstone/revocation and fences future retrieval. It does not
rewrite old Task inputs, historical prompts already sent upstream, or audit history.

## 15b. Demonstration-learned and cross-harness-imported skills — 2026-10-22

Competitors do both: Grok Bot teaches a workflow by watching ("Teach a task"), and Meta's
Muse Code imports skills from Claude Code and Codex with `muse skills import --from
claude|codex`. This section defines how LiteCowork does them **without** weakening §9: both
paths enter as candidates and can only exit as a reviewed `SkillProposal`. A demonstration
or an import is never a skill.

### Provenance

```text
SkillProvenance {
  kind: LITECOWORK_NATIVE | DEMONSTRATION_DERIVED | IMPORTED_FOREIGN | PACKAGE_INSTALL
  source_harness?: "CLAUDE_CODE" | "CODEX" | "OPENCODE" | "GROK_BUILD" | "MUSE_CODE" | "OTHER"
  source_path?: string          # never a secret, credential or private repo path
  content_digest: Sha256Digest  # of every file, at capture/import time
  imported_at?: Timestamp
  observed_session_ref?: Ref    # for demonstration only; not the recording itself
}
```

Every skill keeps this record for its lifetime. An imported skill is re-validated whenever
its digest changes, and a changed digest reopens review rather than silently updating.

### Demonstration capture: what a recording must still become

A recorded interaction contains only the **success path of one instance**. Demonstration
learning research is explicit about the consequence: naive behaviour cloning compounds
error under covariate shift — quadratic regret against linear for interactive correction —
and demonstrations must be **filtered and generalized, not copied**. Translated into skill
terms, a captured demonstration must add all of the following before it may be proposed:

1. **Generalization of literals.** Observed identifiers, filenames, dates, URLs and row
   values become declared arguments with validation, never baked-in values. An
   unparameterized literal is the skill equivalent of a brittle selector.
2. **Decision rules** wherever the demonstration branched, and a stated default when it did
   not branch but could have.
3. **Failure handling.** By construction a recording contains no failure; the proposal must
   name the failure states and the recovery, and must prefer "report the missing source"
   over improvising around it.
4. **Approval boundaries**, kept separate from the procedure: which actions require human
   sign-off, and which the skill may never perform at all.
5. **A test step on safe input**, with the expected output and a validation rule, before the
   skill may be scheduled or automated.
6. **Access preconditions**: which capabilities, connections and resources the skill needs,
   declared so they can be granted and revoked explicitly.

The captured trace is retained only as bounded, owner-inspectable evidence of origin; it is
not replayed, not shipped in the package, and never contains secret bytes. The candidate
then follows §9 gates 1–7 unchanged: redaction, bounded procedure, declared authority,
isolated test, provenance manifest, user review, LiteSPM lifecycle.

### Cross-harness import

The industry has converged on a neutral skill shape, and on `.agents/skills/` as the shared
discovery root that Codex canonically owns and that Muse and OpenCode also read. LiteCowork
therefore imports **`SKILL.md` directories** and treats every other harness convention as a
mapping, not as authority.

```text
importForeignSkill(source_harness, source_path | archive_ref, expected_digest?)
  -> ImportedSkillCandidate {
       parsed SKILL.md, validated against the Agent Skills schema,
       field_translation_report, stripped_body_features[],
       capability_diff,           # what it asks for vs what the user has granted
       warnings[]                 # unparsed fields, executable content, network hints
     }
```

Rules, each traceable to shipped vendor practice:

1. **Schema-validate first.** Parse against the Agent Skills `SKILL.md` schema (`name`,
   `description`, `license`, `compatibility`, `metadata`, `allowed-tools`). Unparsed
   fields are reported, never dropped silently.
2. **Publish the field-translation table, including no-op fields.** This is the single most
   dangerous part of import: `allowed-tools` is honoured by Claude Code, **grants nothing**
   in Grok Build, and is modelled as `permission.skill` patterns in OpenCode. A user
   importing a skill that *looks* permission-scoped must be told, in plain language, when
   the target harness will not enforce it.
3. **Neutralize harness-specific body features.** Dynamic shell inclusion, `@file`
   inclusion, `${VAR}` substitution, bundled `hooks/` and bundled MCP configuration are
   disabled on import unless separately approved — exactly as vendors do for synced skills.
   A bundled MCP server is treated with particular suspicion, because MCP tools are
   conventionally **not** sandboxed by the harness that loads them.
4. **Never auto-grant.** `allowed-tools` is a request, not a grant; every capability routes
   through the ordinary LiteCowork permission flow and an explicit owner decision.
5. **Start disabled.** An imported skill is installed disabled and with model-invoked
   execution off, until reviewed.
6. **Verify integrity.** Fetches are digest-checked before use; archives reject absolute
   paths, `..`, symlinks or hardlinks escaping the skill directory, and decompression
   bombs. Where a publisher publishes a discovery index with per-entry digests, the digest
   is mandatory and a mismatch is a refusal, not a warning.
7. **Treat content as untrusted input.** Skill bodies, their descriptions and any URL they
   fetch are untrusted: a skill cannot grant itself authority, and "remember this" or "you
   may skip approval" inside skill text has no effect on Trust policy.

### Installation is not ours

Upstream LiteSPM already installs `SKILL.md` directories across harnesses **by copy** into
each agent's own skills directory — deliberately not by symlink, so an installation can be
inspected and removed — with a provenance ledger and path-confinement checks. This was
observed in the upstream repository at the pinned 2026-10-22 revision and is recorded in
[LiteSPM interop §5b](LITESPM-LITECOWORK-INTEROP-LLD.md).

A demonstration or an import therefore ends as a reviewed `SkillProposal`, and **LiteSPM
performs the install into each agent's directory**. LiteCowork owns provenance review,
authority declarations, and assigning the installed capability to a Workspace or Coworker —
never the package lifecycle itself. §9 gate 7 already said so; this makes the installer's
existence explicit so no second installer is designed.

### Export

A LiteCowork-owned skill may be exported as a standard `SKILL.md` directory for use in
another harness. Export re-applies the same redaction and authority-declaration rules as
§9, states plainly which LiteCowork-only semantics (grant checks, effect recording,
verification) the exported form **loses**, and records the export in provenance. Export
never includes Connections, secrets, or another Coworker's memory.

## 16. Prompt injection and untrusted-source defense

Automatic learning extraction may use only source classes explicitly enabled by policy.

Default eligible sources:

- direct authenticated owner messages;
- explicit owner feedback;
- user-controlled demonstration metadata.

Default **ineligible** sources for automatic memory commit:

- web pages;
- emails from third parties;
- Slack/Teams messages from other people;
- imported documents;
- MCP/tool descriptions;
- generated model output;
- worker output;
- connector event payloads.

Those sources may contribute Task content, but they cannot say "remember this" and thereby
alter the Coworker.

## 17. UI

### Quiet behavior

The normal chat should not show a modal every time memory changes.

When policy permits auto-commit, an important change may show a subtle item:

```text
Remembered: You prefer weekly reports as a one-page summary.  [View]
```

`View` shows source, exact stored value, scope, certainty, and Undo/Correct.

### Review inbox

```text
Learning
  Needs review
    "Use source links in every research report"
       Instruction change
       [Review diff] [Dismiss]

    "Turn the invoice-cleanup method into a reusable skill"
       Skill proposal
       [Review & test] [Dismiss]
```

This is secondary to Chat. Do not make Coworker home a learning dashboard.

## 18. API target

```text
GET  /v1/coworkers/{id}/learning-candidates?state=&kind=&cursor=&limit=
GET  /v1/coworkers/{id}/learning-candidates/{candidate_id}
POST /v1/coworkers/{id}/learning-candidates/{candidate_id}/approve
POST /v1/coworkers/{id}/learning-candidates/{candidate_id}/dismiss

GET  /v1/coworkers/{id}/learning-policy
POST /v1/coworkers/{id}/learning-policy/revisions

GET  /v1/coworkers/{id}/memory?cursor=&limit=
POST /v1/coworkers/{id}/memory/{memory_id}/revoke

POST /v1/tasks/{task_id}/feedback
POST /v1/conversations/{conversation_id}/messages/{message_id}/feedback
```

Every mutation uses the standard Idempotency-Key/RequestId and expected-version rules.

## 19. Events

```text
learning.candidate.created.v1
learning.candidate.classified.v1
learning.candidate.status.changed.v1
learning.memory.committed.v1
learning.memory.revoked.v1
learning.instruction_patch.approved.v1
learning.feedback.recorded.v1
coworker.learning_policy.revised.v1
```

Event payloads contain IDs/digests/reason codes, not full candidate text.

## 20. State machine

```text
NEW
  -> CLASSIFIED
      -> AUTO_COMMIT_ELIGIBLE -> COMMITTED
      -> NEEDS_REVIEW -> COMMITTED
                      -> DISMISSED
                      -> REJECTED
      -> REJECTED

NEW/CLASSIFIED/NEEDS_REVIEW/AUTO_COMMIT_ELIGIBLE
  -> SUPERSEDED
```

`COMMITTED`, `DISMISSED`, `REJECTED`, and `SUPERSEDED` are terminal for that
candidate. A changed proposal creates a new candidate.

## 21. Failure/edge cases

| Case | Required behavior |
|---|---|
| User corrects memory while extraction in flight | Current source version wins; stale candidate superseded |
| Same correction repeated | Dedupe by normalized semantic key + source lineage; no duplicate memory |
| Inferred memory conflicts confirmed | Review required |
| Temporary chat later becomes Task | Still no learning from the temporary-origin messages |
| Secret detector uncertain | Fail to review/reject; never auto-commit |
| Candidate payload Resource missing | Candidate DEGRADED/REJECTED; never infer content from digest |
| User revokes source Resource | Future extraction stops; active memory remains until explicit memory policy/revocation says otherwise |
| Coworker archived | No new learning commits; history readable |
| Coworker revision changes before instruction approval | Approval stale; rebase/review again |
| Skill test passes once | Still requires package review; one pass is not broad correctness proof |
| Native agent edits its own config externally | Drift is detected; LiteCowork does not import it as Coworker truth automatically |
| External message says "remember this" | Ineligible source; no automatic candidate |
| User says "do this every day" | Automation proposal, not memory |
| User says "never send without asking" | Instruction/Trust proposal; no silent permission mutation |
| Learning classifier unavailable | User work continues; learning can be skipped/deferred |
| Candidate queue large | Bounded batch processing with retention; chat path never waits on learning |

## 22. Tests

1. confirmed preference auto-commit under enabled private policy;
2. inferred preference under REVIEW_ALL remains pending;
3. web/email prompt injection cannot create memory;
4. token/password-like text is never persisted in candidate payload;
5. correction supersedes prior inferred memory;
6. conflicting confirmed memories require review;
7. instruction patch shows exact diff and rejects stale base revision;
8. SkillProposal redacts Task-specific paths/secrets and requires isolated tests;
9. Automation proposal remains inert until normal Automation enable;
10. temporary chat never learns;
11. Coworker handoff excludes private memory;
12. archive fences learning;
13. event payloads contain no candidate body;
14. UI can undo/revoke memory without rewriting historical Task state;
15. LearningService outage does not fail ordinary conversation/task completion.

## 23. Implementation boundaries (logical, not a fictitious crate tree)

Do **not** create a second `crates/application/`, `crates/ports/` or
`crates/domain-learning/` tree merely because earlier target notes named one.
Those paths do not exist in the current workspace. The domain policy, transition
algorithms and classifier interfaces belong to the existing responsible owners:

| Role | Current existing landing zones / integration decision |
|---|---|
| Coworker learning policy, candidates and eligibility | `crates/domain-responsibility/` and existing `storage-core`/`storage-sqlite` |
| Revisions, memory and retrieval scope | existing `domain-rag`, Resource/Context and scoped storage owners |
| Authenticated commands, candidate generation and revocation | `apps/litecoworkd/` existing operator/service contracts; add typed ports only where required |
| User-facing learning/memory and feedback | `apps/litecowork-ui/src/` Coworker/Resources/conversation modules |
| Isolated skill tests and publication | E10 SkillProposal / LiteSPM boundary; do not invent `crates/ports/skill_test_runner.rs` as implemented |

Classifier implementations may be replaceable; deterministic domain rules retain
one canonical owner. New crate extraction requires a migration/ownership ADR,
source implementation, tests and removal of duplicate writers.

## V1 detailed design integration — 2026-10-12

V1-FUNCTION-FILE-CATALOG.md assigns Coworker memory candidate/extraction/conflict/revocation functions and DESKTOP-SHELL-LLD.md maps Settings/Memory controls. User-authored Workspace ContextDocument and automatically learned memory remain distinct. No learning from temporary chat, no automatic sharing, no provider data purge claim before a real receipt.

## Native-agent-owned low-latency memory interface — V1

[Agent Memory Fast Path](V1-AGENT-MEMORY-FAST-PATH-LLD.md) specifies a qualified agent-invoked, scoped search/fetch tool and bounded retrieval/caching. It does not replace this document's candidate eligibility, native memory ownership or purge policy. Automatic injection is opt-in and pinned to accepted Task/Workspace context; an agent need not search, and we cannot promise sub-millisecond uncached provider retrieval. User-facing Memory page may adopt a search/add/empty state while preserving exact Workspace/Coworker/provenance/revocation distinctions. TLM-024–034 are targets, not proof.

## Reconciled implementation landing zones (2026-10-22)

The originally proposed `crates/domain-learning/`, `crates/application/`,
`crates/ports/` and `crates/adapters/` names in historical sketches **do not
exist in this repository**. Those names are conceptual, not evidence of source.
Prefer existing `domain-responsibility`, `storage-core`, `storage-sqlite`,
`domain-rag`, `litecoworkd` and `litecowork-ui` owners unless a deliberate
migration establishes a new crate and prevents duplicate writers. Target §18 API
routes and §19 events require actual producers, router mounts, stored state and
revocation/system tests before a served or qualified claim.
