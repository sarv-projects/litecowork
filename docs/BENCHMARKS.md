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
