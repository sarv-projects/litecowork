# Current architecture audit and development refinements

## Evidence and limits

Baseline is the current tracked repository, not pasted historic hashes. The 2026-10-10
modular re-audit consumed every line of all 119 tracked Markdown files (37,961 lines before
this correction pass), assigned every document to an explicit review module, and then
checked module-local claims and cross-module seams against current source. Machine JSON,
OpenAPI and SQL contracts remain structurally enumerated and validator-checked. A full-line
read is not a claim that green automation proves every semantic property; current behavior
claims are additionally checked against source and recorded test evidence. The repository
now contains a substantial Tauri/React desktop app and `litecoworkd`; current-state docs
must therefore distinguish implemented source, qualified behavior, target design and
historical snapshots instead of assuming no application exists.

[Inventory](audit-inventory.csv), [machine objects](machine-inventory.json), and
[coverage](coverage.csv) are the reproducible review map. The code intelligence index is
local/ignored and syntax/regex based here; it does not provide compiler semantic evidence.
Every story requires a deeper reread of its owning contracts and exact machine definitions.

## Findings and decisions

| Finding | Development refinement | Work |
|---|---|---|
| Durable core is coherent, but documentation size is much larger than current implementation | Preserve identities/transaction/fencing contracts; build one observable vertical slice at a time | E01–E03 |
| Existing early stage contains too many infrastructure deliverables | Keep acceptance scope but split into reviewable stories; ship desktop alpha before cloud | ROADMAP and E01–E10 |
| ACP-first text could imply harness features must fit ACP | Qualify native supported interfaces first, use ACP only when faithful | SP04 / E03 |
| Semantic RAG deliberately outside Core, but file/folder/ZIP experience needs delivery details | Integrate Resource-backed parser/retrieval provider with exact revision/citation/delete contracts | E06 / RAG |
| Cloud/remote appear before local responsibility/office feature completeness in old sequence | Reorder delivery to complete desktop workflows, then cloud, then remote | E11–E12 |
| Existing benchmark mobile scenarios conflict with desktop-only release client scope | Execute same domain/channel continuity with desktop/second desktop and source channel; native-mobile rendering deferred explicitly | coverage variants in TESTING |
| LiteSPM actual wire authority still unavailable | Contract fixture tests plus real integration dependency; never claim production package conformance from fixtures | E04-S04 |
| Motion/status rules are mature | Implement from committed projection states and validate actual-event sequencing | E02 / E08 |
| Broad provider promises risk being untestable | Publish supported-feature matrices by exact adapter/OS/provider version | E03/E07/E13 |
| Green architecture CI is specification evidence only | Add plan integrity checks plus code, system, real-use evidence gates | E01/E13 |
| Competitor documentation changes rapidly | Separate retrieved vendor evidence, vendor examples, inherited links and our proposed parity | SOURCES / WORKFLOWS |

The prior competitive snapshot incorrectly described 2026-10-06 as two days after
2026-10-05. Correct the arithmetic and treat any rollout statement as date-sensitive,
not proof of account availability. External product behavior is not a normative dependency.

## Coverage discipline

The generated coverage inventory is the count authority rather than a hard-coded historic
range. At this audit it reports 137 numbered flows and 90 benchmarks, with all current
flows/benchmarks linked to stories. Coverage assignment selects a primary owner by the
concern in the actual title; an implementer must expand cross-domain dependencies and
execute every assigned scenario, not just its headline.
All normative docs and machine files have primary owners. A primary story owning API or
SQL contracts is a cross-cutting guardian; each domain story still implements its own
operations/constraints/event payloads. The machine inventory is the detailed checklist
for that expansion. New operations/events/flows/benchmarks must update coverage.

## Remaining evidence-dependent choices

The core development toolchain is now pinned and the current local adapter uses rusqlite;
those are no longer open audit choices. Remaining evidence-dependent decisions include
production driver capacity/backpressure qualification, first fully qualified native agent
adapter, semantic retrieval/vector provider, OCR/parser stack, local-model hardware matrix,
supported OS/provider combinations, cloud vendor/placement, signing credentials, and
numeric SLOs. Recommendations are concrete, but unresolved choices are not represented as
already benchmarked architecture facts.
No new autonomous planner, memory database inside Core, or goal-execution authority is added.


## 2026-10-10 full modular line audit

The final post-correction pass consumed **119 tracked Markdown files / 38,123 lines /
2,542,246 bytes**. Every tracked Markdown file was assigned to exactly one review module;
unassigned documents: **0**. Relative Markdown link check from the same pass found **0**
broken local targets. The regex/invariant pass is a candidate generator, not the semantic
decision: every remaining hit was reviewed in context and was a negative/safety statement,
legacy-current-state explanation, or explicitly future target rather than an unresolved
normative contradiction.

| Module | Files / lines | Audit result |
|---|---:|---|
| 00 Governance/core | 8 / 1,654 | Corrected root README from coding/Coworker-mandatory positioning to general desktop cowork with optional Coworkers; corrected stale audit counts/current-app assumptions and closed already-decided toolchain/storage choices. |
| 01 Product/Coworker/UI | 13 / 3,297 | Workbench rules remain consistent: closed on new chat, user-controlled Start shell, no background auto-open, Terminal conditional. Updated desktop and Artifact READMEs to current source reality and pinned toolchain. |
| 02 Agents/delegation | 3 / 1,278 | No unresolved authority conflict found. Native harness owns reasoning/subagents; host delegation remains explicit bounded Task/Attempt work; no generic provider/model router introduced. |
| 03 Capabilities/trust/security | 7 / 1,997 | No unresolved authority conflict found. Catalog/install/auth/assignment/grant/activation/invocation remain distinct; assignment is a ceiling, not authority; large inventories stay lazy/bounded. |
| 04 Task/runtime/environment/recovery | 7 / 3,231 | Core Task/Attempt/Effect/recovery ownership remains consistent. Added explicit post-V1 label to cloud/Hub cells in sleep/offline table so local desktop availability cannot be overstated. |
| 05 Responsibility/context/automation | 5 / 1,441 | Corrected stale domain README: Goal adapter/routes/UI and Automation persistence/manual-run slices now exist; recurring/provider TriggerCoordinator, full presence/health and system/owner qualification remain open. Automatic memory remains target-only. |
| 06 Data/API/events/storage/flows | 8 / 13,794 | Proposed Coworker-owned Conversation, memory, assignment and StandingResponsibility records remain clearly target-only. Corrected stale Storage claim: desktop now exposes one-shot ManualTrigger **Run once**; recurring hosting/settlement remains unavailable. |
| 07 Testing/observability/research | 5 / 1,544 | No unresolved contract conflict found. Competitor material remains dated evidence, not runtime truth; release evidence still requires CODE + SYSTEM + USER qualification. |
| 08 Implementation plan | 16 / 5,619 | Corrected stale AUDIT state and current decision list. Roadmap done/not-done matrix remains source-grounded; target docs do not promote story status. |
| 09 Epics/reviews/spikes | 22 / 3,623 | Corrected E07 header to match backlog (S00/S01 IN_PROGRESS, later stories planned). Historical review/current-run entries remain historical rather than rewritten as current authority. |
| 10 ADRs | 25 / 645 | ADR set is internally consistent after marking ADR 0018's memory-learning target as amended by ADR 0024; Resource-backed provenance/revocation decision remains active. |

### Cross-module seam conclusions

1. **Conversation ↔ Coworker:** ordinary Conversation/Task origin can be Coworker-free.
   The existing Workspace primary Coworker is explicitly a legacy/interim optional
   Task-composer preference, not first-run identity or Conversation ownership.
2. **Coworker ↔ execution:** a Coworker is identity/context/responsibility configuration,
   never a second planner, scheduler, model runtime, Task engine, or permission authority.
3. **Connections ↔ Trust:** installed/authenticated/assigned/ready/granted/activated/
   invoked remain separate. Task-time Connect may repair a blocker but cannot replay an
   ambiguous Effect or mint authority.
4. **Memory ↔ Knowledge:** current Resource-backed manual ContextDocuments are real partial
   source; automatic quiet private memory is an accepted future candidate/policy pipeline.
   Connected knowledge is not automatically durable memory.
5. **Responsibility ↔ Automation ↔ Task:** StandingResponsibility is target-only grouping.
   Existing manual Automation occurrence can create one READY Task; recurring/provider
   trigger hosting is not qualified. TriggerHost never owns Task execution.
6. **Runtime ↔ cloud:** desktop/local V1 is the release target. Cloud/Hub/remote behavior in
   architecture documents is post-V1 and cannot be used to claim laptop-off continuation.
7. **Workbench ↔ authority:** Workbench is presentation/control hosting only. Opening a
   Browser, Terminal, Artifact, MCP App or Computer surface neither grants capability nor
   proves work succeeded.
8. **Target ↔ current source:** proposed schema/routes/events remain visibly proposed until
   canonical machine contracts and code land. Historical implementation notes may describe
   what was unverified at that date; present-tense status documents must reflect later
   commits/tests.

### Remaining intentional gaps after the audit

Provider-backed ordinary chat/turn admission; Coworker-owned Conversation + last-active
projection; Coworker capability assignment; automatic MemoryCandidate/CoworkerMemoryPolicy;
StandingResponsibility persistence/API; scheduled/provider/event TriggerCoordinator;
deterministic proactive monitoring; unified Workbench shell and Browser/Computer/Terminal
integration; complete notification delivery; Coworker-to-Coworker handoff; Windows
authenticated Operator transport; full local Task/Attempt dispatch with atomic
Trust/Invocation/Effect admission; and supported-OS/provider SYSTEM + USER qualification.
These are implementation gaps, not documentation ambiguities.
