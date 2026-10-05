# LiteCowork Real-Work Benchmark

Benchmarks are product acceptance scenarios, not synthetic agent-only puzzles. Each records Task completion, evidence quality, human interventions, recovery behavior, cost/usage, artifacts and time.

## Core benchmark families

### B01 Persistent assistant
Read school/work email, identify obligations, update calendar/todo through capabilities, produce daily brief. Test duplicate-message safety and approval boundaries.

### B02 Local document research
Research 20 local PDFs plus web sources and produce cited structured report. Test progressive retrieval, no huge prompt dump, artifact provenance.

### B03 Large data quality
Analyze millions of product-feed rows, partition work, delegate low-ambiguity checks, merge deterministically and produce correction report.

### B04 Finance package
Update forecast model -> create/edit workbook -> derive board slides -> verify formulas/rendering -> publish artifacts.

### B05 Sales meeting prep
CRM + email + meeting notes -> prioritized brief and open questions. Test connected capability discovery and evidence links.

### B06 Marketing operations
GSC/crawl/ads data -> analysis -> recommendations -> implementation tickets. Test reusable Skill creation after successful run.

### B07 Browser fallback
Start with connector/API; fall back to DOM/browser only for unsupported action; computer use only when required. UI must represent actual method truthfully.

### B08 Files/BOM
Messy industrial/project folder -> classify documents -> master BOM/maintenance list -> verification against source files.

### B09 Office artifact
Raw data -> editable XLSX/PPTX/DOCX/PDF through external Office capability; Workbench displays versions; render verifier detects broken layout.

### B10 Coding team
Codex/Claude lead -> OpenCode child worktrees -> independent test/verifier -> merge diff Artifact. Kill one child and recover without losing parent Task.

### B11 Heterogeneous research swarm
Lead delegates independent bounded research to multiple agents, enforces concurrency/depth/budget, rejects conflicting/poor child result and integrates verified synthesis.

### B12 Weekly recurring research
Remember previous findings through explicit Task/artifact state, avoid duplicate topics, create weekly report, send only when useful.

### B13 Remote channel
Start Task from Telegram/email while laptop asleep; cloud executes; user checks status from phone; artifact appears on desktop later.

### B14 Cross-device continuation
Desktop -> explicit cloud handoff -> mobile steering -> desktop reconnect. Same Conversation/Task; no process-teleport fiction.

### B15 Failure recovery
Kill lead process, provider, Runtime and network at controlled phases. Verify recovery ladder and stop criteria.

### B16 Ambiguous side effect
Send email/update CRM, cut connection after request leaves. Reconcile before retry and demonstrate no duplicate mutation.

### B17 Capability discovery
Task requires capability unknown at start; agent searches LiteCowork Gateway, LitePSM resolves exact package, grant/approval occurs, provider activates, Task completes.

### B18 Skill learning
Successful repeated procedure -> draft SKILL.md -> remove task-specific secrets -> user review -> LitePSM-managed version -> next Task uses pinned skill.

### B19 Human takeover
Agent controls a browser/desktop Environment; user takes control with the current
EnvironmentControlLease epoch; queued agent input is discarded and old-epoch calls are
rejected. LiteCowork observes the human-modified state, reconciles drift/Effects, then
returns control only under a new Agent epoch. Verify no action queued before takeover is
replayed and the Runtime ExecutionLease is not confused with the input-control lease.

### B20 Verification honesty
Worker claims output complete but file missing/layout broken/test failing; LiteCowork refuses COMPLETED until verifier passes or user accepts exception.

### B21 Persistent folder context
Add a local folder as a WorkspaceRoot once, then search it from a later Task without
reattaching it. Check stable Resource identity across local/cloud copies, bounded
deterministic search with zero model tokens, no implicit context attachment, watcher-gap
freshness, and identity revalidation after Runtime reconnect.

### B22 Pause versus cancel
Pause a Task with active workers and an open Effect. Verify new Attempts stop, workers
reach safe boundaries, ResumePacket is committed, Effects reconcile, leases release, and
Task becomes PAUSED only when safe. Resume must create fresh Attempt/lease epochs. Repeat
with an ambiguous Effect and confirm pause reports a blocker rather than claiming success.
Repeat while a TASK_PLANNING session is active: pause/cancel must close it, reject a late
PlanRevision after the Task-version transition, and never leave a Step created after the
pause/cancel fence.
Repeat with a long-running MCP Task: cancellation acknowledgement alone must not permit
`PAUSED`; a provider-confirmed `input_required` invocation may remain quiescent, with its
UserRequest deferred until resume. Also cancel from both `PAUSED` and `PAUSE_REQUESTED`;
verify cancellation supersedes pause, pending Task-scoped UserRequests and Approvals close
through their owning services, stale responses cannot restart work, and cancellation still
waits for provider operations, VerificationRuns, and ambiguous Effects to settle.

### B23 Asynchronous capability call
Invoke a long-running read-only MCP operation that returns a Tasks-extension handle. Kill
the AgentSession, resume polling from the durable CapabilityInvocation, handle
`input_required` through UserRequest/`tasks/update`, and prove the MCP task ID is not a
LiteCowork Task. For Streamable HTTP, verify `Mcp-Method` and `Mcp-Name=taskId` routing on
all follow-up calls. Test provider expiry and cancellation acknowledgement without
assuming the operation stopped. Lose authoritative provider state while the invocation is
WAITING and INPUT_REQUIRED; record AMBIGUOUS and require reconciliation before pausing,
cancelling, or retrying.

### B24 Requirement and evidence pinning
Change an acceptance criterion while preserving its ID and change an input Resource
revision. Verify old VerificationRuns cannot satisfy the new requirement and dependent
Artifact/Evidence projections become stale without mutating immutable history.

### B25 MCP Skill manifest and origin safety

Connect two MCP servers publishing same-named Skills, reference a Skill URI absent from a
partial listing, change one manifest after approval, serve a mismatched digest/size/
frontmatter, include a nested Skill, expose an unlisted directory child, and advertise a
dynamic Skill. Verify origin-qualified identity, explicit `skills/get` confirmation,
directory-read negotiation, no manifest expansion, no shadowing, content-bound approval
invalidation, cross-origin read isolation, explicit nested consent, and v1 refusal to
activate dynamic content.

### B26 One-time approval replay
Deliver the same approved command twice and attempt to reuse the approval against a
different request digest. Exactly one ApprovalUse may commit; duplicate same-request
delivery returns the deduplicated result, while changed scope/digest is rejected.

### B27 Notification is not completion
Deliver a Task notification successfully while the Task remains RUNNING, then fail
notification delivery after the Task completes. Task truth must remain independent from
channel acknowledgement and delivery retry.

### B28 Backup and restore
Restore a Workspace from a verified database snapshot, event cursor, and blob manifest.
Verify integrity, current projections, artifact access, open-effect reconciliation, and a
new Runtime identity. Prove that restore never revives a prior ExecutionLease epoch.

### B29 Pause/cancel during verification
Race pause and cancellation against a Task in VERIFYING while one VerificationRun is
active. Confirm the Task aggregate version serializes the outcomes, no late verifier result
can complete a Task after pause/cancel commits, active runs remain bound to their criterion
and input digests, and pause/cancel stays pending until each run settles or reaches its
bounded INCONCLUSIVE timeout.

### B30 Persistent Environment provision and reuse

Preview several Runtime/provider offers for a persistent Environment. Check cost-estimate
confidence, provider versus host budget enforcement, resource/network limits, source pin
freshness, retention, and backup implications. Create only after explicit confirmation;
reboot or suspend/resume it; use it from a later Task with fresh grants; then destroy it
after all consumers/effect/checkpoint holds settle. Unknown provider state blocks duplicate
provision/destroy. Workspace archive retains the Environment safely suspended. At the
cumulative ceiling, confirm new use stops, affected Steps become `BLOCKED`, and there is no
in-place budget update/reset. Preview eligible alternatives, choose one by candidate ID and
plan digest, and verify a fresh Attempt binds to it only after atomic revalidation. Also
provision a replacement and verify that private provider state is not implicitly cloned.

### B31 Needs You inbox consistency

Create an Approval and blocker linked to it, a UserRequest and blocker linked to it, plus
an independent actionable blocker. Confirm the inbox shows three stable source identities,
not five rows. Retry/fail notifications without changing counts. Resolve each underlying
record and confirm stale cached counts are marked stale and refreshed on reconnect.

### B32 Native Claude harness integrity

Run the supported Claude path with native user configuration, skills, hooks, MCP,
permissions and subagents. Verify LiteCowork adds only the negotiated bridge and records
no private prompt, handle, or config bytes.

### B33 Native Codex harness integrity

Run the qualified Codex harness path with native configuration, tools, sandbox and
approval behavior. Verify integration does not flatten or silently replace supported
native semantics.

### B34 Native OpenCode harness integrity

Preserve OpenCode agents, permissions, skills, MCP and subagents through the adapter.
Change normalized config while warm and verify re-probe/fail-closed behavior.

### B35 Same binding, premium lead and cheaper worker profile

Use one AgentBinding with separate supported lead and worker session options. Verify
separate AgentSessions/Attempts and explicit failure for an unsupported model override.

### B36 Heterogeneous lead-to-worker delegation

Run a premium lead with a different eligible worker. Verify accepted-Plan Step admission,
child grant isolation, bounded TaskPacket, new lease, and result verification.

### B37 Cost-first worker selection

Provide deterministic, local, cheap external, and unknown-cost candidates. Verify hard
filters precede ranking, unknown cost is not zero, and quality floors remain satisfied.

### B38 Quality-first selection

Provide workers with different verifier pass rates and costs. Verify quality-first ranks
only eligible candidates and low-confidence samples do not overrule explicit choice.

### B39 Verification-triggered escalation

Fail a cheap worker's verifier, reconcile Effects, and admit a stronger profile as a new
Attempt. Verify bounded escalation, preserved provenance, and no in-place worker change.

### B40 Profile disabled during active child

Disable/revise a profile while a child runs. Verify no later admission uses it, the
current child retains its pinned revision, and explicit cancellation settles safely.

### B41 Native configuration drift during warmth

Change agent configuration while a host is warm. Verify descriptor refresh,
`AGENT_NATIVE_CONFIG_CHANGED` for stale admission, and no native file rewrite.

### B42 Low quota fallback prewarm

Report a source-backed LOW observation. Verify fallback host/auth/config/Environment can
be prepared without model invocation, Attempt, grant, or lead change; evict it and prove
Task correctness is unaffected.

### B43 Quota exhaustion lead handoff

Compare provider-confirmed EXHAUSTED with unavailable/UNKNOWN. Verify only explicit policy
or owner action changes lead and the fresh session receives a bounded durable handoff.

### B44 Warm host with fresh native session

Keep a host warm after one Task settles and start another. Verify fresh session,
config/auth, no cross-Task native history, and no inherited grants or leases.

### B45 Warm browser with fresh authority

Reuse a Coworker-private browser Environment for a later Task. Verify provider state is
revalidated while Task/Attempt grants, approvals, Effect policy, and input control lease
are fresh.

### B46 Parallel worktree isolation

Run Claude/Codex/OpenCode writers concurrently against one source tree. Verify isolated
worktrees/overlays, deterministic merge/conflict handling, and no lost updates.

### B47 Shared browser control fencing

Race two workers and a human takeover against one EnvironmentControlLease. Verify only the
current owner/epoch can act, queued old input is dropped, and control differs from
ExecutionLease.

### B48 Deadline-sensitive preflight

Vary Runtime, auth, page freshness, approval, budget, capability, and fallback readiness.
Verify precondition failures happen before Effects and no hard realtime promise/ETA is
shown.

### B49 Structured-to-browser fallback

Remove a structured operation while preserving an equivalent browser path. Verify method
change is authorized and recorded; reject fallbacks that change target/effect meaning.

### B50 ActionBatch execution

Execute a bounded batch with preconditions, postconditions, and abort guards. Verify each
consequential suboperation keeps its own Effect/idempotency/reconciliation/Evidence.

### B51 Human final-authorization boundary

Prepare a consequential operation through preflight, then require takeover before final
submit. Verify no queued action proceeds before the new control/approval epoch.

### B52 Credential invisibility

Use a credential broker for an authorized operation. Inspect agent context, TaskPacket,
logs, events, Resource output, and backups; secret bytes never enter agent-visible or
replicated data.

### B53 Coworker identity survives lead replacement

Change a Task lead and replace a worker while retaining Coworker identity, history,
preferences, and Task continuity. Verify Coworker never becomes an Attempt owner.

### B54 Goal progress derives from verified outcomes

Link completed, active, failed, stale-input, and conflicting Tasks. Verify progress cites
Evidence, stale/conflicted work is marked, and only the owner completes the Goal.

### B55 Suggestion cannot self-authorize

Accept Task, Routine-editor, and Automation-editor suggestions. Verify normal admission or
explicit editor save and no automatic grant, install, send, or schedule.

### B56 ContextDocument concurrent edit conflict

Edit one user-authored Resource from two devices. Verify both branches remain, stale head
returns conflict, and retrieval cannot silently select a branch.

### B57 Demonstration to SkillProposal

Capture semantic browser actions with typed inputs and secret placeholders. Verify trace
review/test gates and no package install/invocation before review.

### B58 Skill drift detection

Change a saved semantic target/site state. Verify drift blocks unsafe replay and creates a
reviewed proposal revision instead of silently mutating the published Skill.

### B59 Routine health and dependency drift

Run a pinned Routine across success/failure/unavailable/changed-dependency conditions.
Verify health is a projection and repair creates a reviewed revision without rewriting
prior occurrences.

### B60 Premium-usage efficiency comparison

Compare (A) premium lead plus premium/native subagents with (B) premium lead plus
qualified cheaper workers and deterministic capabilities. Measure verified completion,
premium usage, total known cost by currency, elapsed time, retries, output quality and
human rescues. Report unknown-cost coverage/confidence; make no “5×” claim without
reproducible results.

### B61 Suggestion snooze and expiry

Snooze a live Suggestion to each supported preset, race a stale-version update, and
advance the service Clock through snooze and expiry. Verify the proposal stays
`PROPOSED` while snoozed, visibility follows persisted time, expiry wins, and no task is
created by snoozing.

### B62 Suggestion-kind mute and cooldown

Mute a kind with multiple open Suggestions and race new proposals. Verify one atomic
preference update resolves current rows with `MUTED_KIND`, suppresses concurrent/future
proposals without storing their source text, and unmute does not revive prior rows. Verify
owner dismissal cools down only the identical dedupe key for 30 days.

### B63 Goal reopen

Complete and reopen a Goal across concurrent version updates. Verify the owner command
appends a status event, retains the earlier completion and progress evidence, leaves
linked Tasks/Routines untouched, and cannot reopen an archived Goal.

### B64 DelegationProfile duplicate and rename

Duplicate a non-archived profile while racing a source revision, profile archive, and a
same-name request. Verify `If-Match` pins the source revision, same-key retries replay the
committed result, conflicting key reuse fails, normalized names are unique per binding,
and the new profile is revision 1 and disabled. Verify it copies only the exact current
non-secret revision, retains the adapter-option descriptor digest for revalidation, and
copies no execution, auth, grants, budget reservations, Environment, or performance state.
Rename through a new immutable revision and confirm prior Attempt provenance retains the
prior name and revision. A stale/archived source, stale descriptor at enablement, and
name collision must not start a worker or alter the source profile.

### B65 Pinned Suggestion provenance

Create a Suggestion from a specific Resource revision and Goal revision, then advance one
source and make another conflicted/unavailable. Verify the Suggestion and event keep the
original pinned references. Acceptance of the unaffected proposal creates an ordinary
Task with the same input revisions; acceptance of a stale/conflicted required source
opens review/update, does not silently advance the ref, and leaves the Suggestion
`PROPOSED`. Confirm Goal progress still cites Task/Evidence and cannot be inferred from
Suggestion or worker text. Revise a Coworker avatar Resource and verify the prior
CoworkerRevision retains its pinned image revision.

### B66 Semantic architecture-contract drift

Mutate one contract at a time: remove an enum member, change a failover field, omit
`lead_eligible`, detach a Coworker revision from an AutomationRevision, mismatch an
Attempt's DelegationProfile ID/revision, change an ActionBatch digest field, omit
`ExecutionMethod` provenance, remove a ContextDocument edit/delete route, or drop a state
transition guard. Verify `validate_architecture.py` fails with a targeted cross-contract
finding before architecture CI can pass. Restore each contract and verify the validator
passes without relying on comments or filename-only checks.

### B67 Lead failover policy and race fencing

Exercise `DISABLED`, `ASK`, and `ALLOW_LISTED` with valid, stale, expired, unsupported,
and wrong-trigger observations. Race TaskSpec revision, manual lead change, quota recovery,
Runtime incarnation change, and fallback disablement against automatic admission. Verify
only an eligible fresh fallback changes the lead; `ASK` and exhausted policy open Needs
You; TaskService fences the old planning session at the expected Task version; the event
pins cause/actor/spec/trigger; Attempts and grants remain with their original provenance.

### B68 Coworker-pinned Automation admission

Create an AutomationRevision that pins a Coworker revision and race a claimed occurrence
against Coworker pause/archive and Automation disablement. Verify no new Task is
materialized if pause/archive/disable wins, the occurrence remains deduplicated and
truthfully blocked/skipped, and a winning Task create atomically pins the exact Coworker
and Automation revisions into Task origin/spec provenance.

### B69 ActionBatch partial settlement and per-member Effects

Admit a batch with read-only and consequential members, then inject a precondition failure,
provider timeout, abort condition, and late success. Verify all members were durably
admitted before dispatch, share one method/digest/count, have independent idempotency and
Effect links, and are fenced individually. Confirm undispatched members stop, earlier
Effects reconcile, ambiguous members block unsafe retry, and no transaction-wide rollback
or synthetic batch Effect is claimed.

### B70 Resource revision concurrency and explicit merge

Start two revision uploads from the same exact head set; commit one and then the other.
Verify the stale commit returns `RESOURCE_CONFLICT`, does not append a ResourceRevision or
advance the head, and cannot silently rebase. Merge only after the owner names all current
heads; verify the resulting DAG, digest, event, and Task input all pin the intended
revision.

### B71 ContextDocument purge manifest completeness

Delete a ContextDocument with multiple blob/index replicas, stale Runtime incarnation,
retrying and duplicate acknowledgements, an unregistered provider, and an empty target set.
Verify the immutable manifest pins ordered exact revision targets and count; no receipt
outside the plan is accepted; `DELETED` is impossible before exact acknowledged set
equality; stale incarnations cannot acknowledge; the verified empty plan can complete;
backup restore preserves tombstones/receipts but does not restore acknowledged content.

### B72 Suggestion producer provenance and suppression

Run registered deterministic and read-only producers with current, stale, cross-Workspace,
muted, duplicate, dismissed-cooldown, and expired candidates. Verify only validated
candidates become durable Suggestions with `proposed_by`; suppressed source text is not
stored or evented; accepting a Task creates ordinary Task state and cannot bypass Trust.

### B73 Demonstration capture pause and hard bounds

Exercise action, byte, and duration caps; sensitive-region detection under both policies;
manual pause/resume; Environment lease loss; and Runtime restart. Verify counters never
exceed the immutable capture policy, no secret fields enter trace Resources, pause requires
fresh same-Environment authority, conversion creates only a SkillProposal, and published
Skills still require their ordinary review/install lifecycle.

### B74 Execution-method provenance integrity

Create Invocations with each supported method and with `UNKNOWN`, exercise ActionBatch
members, Effects, fallback, and forged agent-reported route text. Verify the adapter pins
the method before Invocation creation, dispatch repeats it, SQLite rejects method mutation
and cross-method Effect links, each batch shares one method, and the UI never upgrades
`UNKNOWN` to an inferred API/browser claim.

## Failure/edge benchmark extensions

- OAuth expires mid-Task
- Google Drive file changes after read
- streamed/cloud placeholder file not materialized
- two workers target same spreadsheet
- two workers target same git checkout
- duplicate scheduled occurrence
- condition watch with no change sends no notification
- TaskSpec changes while child is running
- package/Skill updates while Task is running
- subagent lacks requested model/capability
- 100 child delegation request hits limits instead of swarm explosion
- Hub offline while local Task continues under policy
- Runtime reconnects with stale fence
- capability unhealthy after discovery
- cloud lacks required local Chrome/session/secret
- DNS rebinding/redirect attempts target IPv4/IPv6 private or metadata addresses
- local MCP Gateway token replay from another process or after session revocation
- MCP App requests an undeclared domain or ungranted tool
- conflicting chunk retry reuses an upload range with different bytes
- upload/archive parser encounters traversal, symlink escape, or decompression bomb
- simultaneous Workspace instruction revisions are pinned to the correct Task specs
- current MCP Tasks extension expires provider state during a LiteCowork AgentSession loss

## Scoring dimensions

Do not collapse to one leaderboard score. Record:
- outcome success
- mandatory criteria passed
- evidence level per criterion
- duplicate/unsafe effect count
- human interventions
- recovery count
- total Attempts/child Attempts
- wall time
- usage/cost where observable
- artifacts produced
- user-visible truthfulness defects

## Lifecycle and reusable-work scenarios

| Scenario | Required evidence |
|---|---|
| Boot with many installed agents/providers | Ready daemon, live authorized watchers/scheduler, no unrelated worker launch |
| Concurrent turns and idle host teardown | One compatible host admission, live references protected, bounded owned cleanup |
| Save weekly review as Routine and schedule it | Reviewed immutable Routine revision, explicit Automation confirmation, exact Task pins |
| Cloud schedule needs laptop Excel | One occurrence waiting on named dependencies; no ineligible cloud execution |
| Sleep through several schedule slots | Pinned misfire/DST behavior, cursor continuity, bounded deduplicated catch-up |
| Stop local Runtime while work runs | Dependency preview, checkpoint/reconciliation or honest blocker, cloud ownership preserved |
| Reboot with user-opened apps and persistent VM | No user-app termination; provider reattachment with fresh identity/control authority |

Record these separately from implementation throughput benchmarks; a successful outcome
without correct ownership, deduplication and truthful UI fails the scenario.
