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
Agent uses browser/desktop capability; human takes over; LiteCowork stops automated input, resnapshots state, then resumes safely.

### B20 Verification honesty
Worker claims output complete but file missing/layout broken/test failing; LiteCowork refuses COMPLETED until verifier passes or user accepts exception.

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
