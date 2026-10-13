# V1 Deep Design Reconciliation and Dry-Run Request Trace — 2026-10-13

Status: a manually adjudicated **design review** of selected high-risk seams, complemented by a mechanical line-ingestion index of every first-party Markdown file. This document is NOT certification of universal semantic consistency of all prose or successful execution. It does not duplicate canonical schema definitions or introduce new execution authorities.

## Canonical owners and normalized distinctions

| Ambiguous term or duplicate | One authority / decisive contract | Other layers must NOT implement |
|---|---|---|
| Coworker vs native agent | RESPONSIBILITIES / COWORKERS-TARGET own stable identity/preferences; AGENT-CONTROL owns agent binding/native configuration | a second Coworker planning model, duplicated native auth or global model router |
| Ordinary Conversation vs Coworker-owned Conversation | ConversationService owns Turn/Message; target CoworkerConversationService owns immutable owner relationship | primary Workspace Coworker cannot silently own ordinary chats |
| Task vs Conversation vs AgentSession | TaskService for durable work; ConversationService for user text/Turns; AgentSessionSupervisor for provider process | treat transient thinking/text as Task event or Task success |
| Memory vs ContextDocument vs native agent private memory | LEARNING owns candidate/state/auto private commit; ResourceService owns authored ContextDocument bytes/revisions; native agent owns its own memory | duplicate CoworkerMemoryPolicy, MemoryCandidate event/state/DB store, native memory override |
| Routine vs Automation vs StandingResponsibility | RoutineService owns reusable spec, AutomationService + TriggerCoordinator own schedule/Occurrence, optional StandingResponsibility only groups links/policy | extra scheduler, executor or automated planner under ResponsibilityService |
| Capability vs Connection vs Coworker assignment vs Grant | LiteSPM owns installation/connection; Coworker assignment is a ceiling; TrustService grants/approves; InvocationRunner executes | installed/assigned implies actually authorized |
| Provider gateway health vs LiteCowork native usage | Gateway owns its rate-limit/model breakers; LiteCowork owns admitted native/Task receipts and scoped Core-mediated capability circuit | silently take over provider selection/model, claim measured usage when opaque |
| Right Workbench Terminal vs native-agent process | EnvironmentProvider/TerminalSessionService owns explicitly mediated PTY; native agent keeps unmediated private shell | pretend universal terminal takeover/complete process audit |
| User-facing status vs committed state | StateStore/EventStore, trusted source/projection | optimistic progress/animation emitted as durable success |

## Dry run A: ordinary chat versus Coworker chat

Example: user opens normal New Session, selects an agent, asks a question, later opens a Research Coworker and asks a similar question.

| Hop | Normal Session | Coworker Session | Forbidden failure |
|---|---|---|---|
| A1 UI creation | ConversationService.create, owner null | CoworkerService.create followed by same-Workspace CoworkerConversationService.create | Workspace primary Coworker auto-owns ordinary chat |
| A2 idempotency | stable create key, scoped messages | two stable *different* creation keys; retry missing step only | duplicate Coworker/Conversation after lost receipt |
| A3 Agent selection | Conversation override, Workspace default | Conversation override, owning Coworker default, Workspace default | silently falling through an invalid configured agent |
| A4 Native UX | descriptor-driven models, slash/@/attachments | identical native composer with owner scope + role preferences | reduced Coworker-only model wrapper or mixed-agent options |
| A5 Memory | selected Workspace context / explicit sources only | current owner policy + agent-chosen memory tool or approved prefetch | forced global memory search; hidden private agent-memory edit |
| A6 Turn admission | authenticated ConversationService.submit_turn; commit user message and turn | same route after owner-association schema/handler promotion; pin owning identity | persist unsendable turn, or create Task for a normal chat |
| A7 Stream/complete | provisional source-ordered stream, then committed Message and Turn | same, plus source-pinned Coworker recency (only actual conversation activity) | transient tokens become duplicate final message/false activity |
| A8 Learning | ordinary session has no Coworker auto-private learner | qualified owner learning source at committed checkpoint; no blocking user turn | user private text persisted by default without eligibility |
| A9 Switch agent | start new qualified session with scoped context | same; Coworker memory and private provider session remain separate | copy native credential/session or silently switch provider |

**Resolved target contract correction:** API, FLOWS, SERVICES and COWORKERS-TARGET now name the Coworker-owned Conversation precedence and Task source-identity guard; they preserve current ordinary-Conversation routing as a distinct present-day contract. LEARNING/DATA-MODEL own candidate enum and memory policy, not duplicate Coworker types.

**Not promoted:** Coworker-owned Conversation actual Operator route/row/adapter handling. Until shipped, the UI cannot claim Coworker first chat or native Send is functional.

## Dry run B: standing request across Coworker, scheduler, native agent, memory, email and evidence

Example: "Create Research Coworker. Check approved news sources every weekday; give me a Friday briefing, draft a document, and send it to my team after I approve."

| Hop | Gate / owner and exact transition | Error, restart and UI obligation |
|---|---|---|
| B1 Identity | explicit Coworker Create; immutable revision, optional agent binding | no model/session invoked, no required connector |
| B2 Intent | User Message persisted; native chief may produce a structured suggestion only via qualified proposal tool; otherwise user invokes Create from message editor | free-form model text is not a Task, Routine, Automation, Grant or consent |
| B3 Review | user confirms source scope, cadence, IANA timezone/misfire/overlap, deliverable, approved recipients, budget/stop criteria, placement, interaction policy | edits invalidate old proposal digest; no auto-enabled trigger |
| B4 Store | RoutineService owns immutable RoutineRevision; AutomationService owns trigger revision; optional Responsibility groups links and fences new admissions | partial multi-definition save needs durable repair, not a fictitious cross-service atomic commit |
| B5 Activate | TriggerHost acknowledgement/cursor and prerequisite readiness before indicating ENABLED | if one activation fails, leave PAUSED/BLOCKED and fence any already live hosts; no mixed half-enabled group |
| B6 Observe | signed connector event or scheduler tick; source identity, host epoch, dedupe key, deterministic no-change filter | no-change with actual observation is NO_ACTION, not fabricated Task completion; source unavailable is UNKNOWN |
| B7 Materialize | one Occurrence -> one Task with exact approved Routine/Automation/Coworker revisions and source refs | if paused/archived/grant changed at admission, block or skip under policy; never resurrect authority |
| B8 Plan | TaskService + PlanningCoordinator starts native TASK_PLANNING AgentSession; accepted PlanRevision, Steps | no uncontrolled Core planner or duplicate plan authority |
| B9 Execute | scheduler reserves Step/Attempt/Environment/lease; per capability Invocation/Effect approved by Trust | missing LiteSPM connection returns Needs You; no credentials in chat |
| B10 Memory | source-granted qualified memory tool/prefetch reads only current Coworker scope; final revocation check | stale cached hit denied; external native session cannot erase earlier knowledge |
| B11 Handoff | permitted agent worker gets bounded TaskPacket and exact source versions/worker authority | no Coworker private memory or native session token leaked by default |
| B12 Document | ArtifactStore exact immutable output + Evidence/VerificationRun under Task criteria | agent "done" text not enough; missing/old document blocks verified completion |
| B13 Email | explicit draft/review/Approval, managed Invocation+Effect and provider idempotency where possible | a lost provider receipt is AMBIGUOUS, not automatic repeat |
| B14 Result | canonical Task/Run history and one concise Coworker activity update; Workbench opens approved ArtifactVersion | do not spawn hidden Conversation for every scheduled Task |
| B15 Learning | eligible verified Task outcome triggers bounded LearningCandidate, private scope and policy gating | generated text or third-party page cannot auto-become confirmed memory |
| B16 Analytics | distinct real inference leaves with source/provenance and separate measured/estimated/unknown cost | no double count from parent+child or native+gateway; no hard currency guarantee if opaque |

**Design gap still requiring schema promotion:** qualified native proposal-to-review adapter and authoritative multi-Automation activation receipt/outbox must be implemented and tested; a prose description of a user request cannot substitute for those interfaces.

## Dry run C: debugging request with terminal, Chrome, takeover, changing native agent

Example: "Fix the failing test in my Workspace, inspect it in Chrome, then let me take over the browser."

1. Composer keeps file attachments and native slash/@ tokens pinned to the actual selected agent descriptor; changing OpenCode to Codex invalidates incompatible tokens/options while preserving the draft.
2. Consequential code edits and shell actions require Task/Attempt/Effect admission; an ordinary read-only Conversation is not a secret general-purpose shell executor.
3. Runtime and Environment readiness precede terminal PTY/ConPTY creation. Shell profile, exact CWD, Windows/WSL Runtime identity, process incarnation and owner are visible.
4. Native harness-private commands remain native; a separate right-Workbench terminal is mediated by EnvironmentProvider. Unmediated native shell cannot truthfully expose Task-level approval or Take Control guarantees.
5. Browser is real qualified Chrome/Chromium provider, not merely Tauri WebView. Managed profile/attach consent, tab incarnation, origin grant, control-epoch and action evidence are checked for every mutating action.
6. Browser user takeover CAS fences queued agent input. Return requires fresh observation and policy; old selectors and inferred DOM state are never replayed.
7. Tool/Effect timeout after page submit/command write -> UNKNOWN/AMBIGUOUS. Reconcile before retry, never double execute a consequential operation.
8. Rendering opens exact ArtifactVersion/diff/source; focus, unsaved file draft and browser/terminal tabs survive panel switches. A tab close does not silently stop a Task.
9. Success is tied to test/evidence/verification and current source revisions, not terminal OSC command markers, provider "done" text or UI animation.
10. Health view separates OS/browser/PTY health, AgentSession outage and Task/Effect settlement; opaque model usage stays UNKNOWN.

## Dry run D: sleep, quota, offline, cancellation and cross-Coworker handoff

- A due trigger on a sleeping local machine cannot execute there. On wake TriggerCoordinator evaluates exact missed slots, misfire, current grants, finite budget and source freshness. Do not display completed-at-due-time if no work ran.
- Provider 429 with reset time is account/model scoped if actually observed; gateway breaker is source owned. Native agent quota unknown does not become a guessed remaining token count or guaranteed spend ceiling.
- An external send may have succeeded despite a daemon crash. Persisted Effect identity and provider reconciliation block automatic duplicate.
- If Research Coworker hands a briefing to Writing Coworker, user-approved exact ArtifactVersions travel into a **new destination Task** with target Coworker revision, lead, grants and context. No private memory, hidden native session or source credential moves.
- A source revoked during asynchronous memory search fences retrieval after cache hit and before attachment. Revocation does not retroactively delete previously observed provider bytes; provider purge must report actual receipts.
- Notification success/failure is separate from Task status. An offline email/push channel does not rerun the Task or approve a pending action.
- Two windows racing on AgentBinding, approval, terminal input or Coworker edit must obey canonical expected-version/epoch and one authority; stale UI requests return conflicts, not false success.

## Gaps found and their actual disposition

| Finding | Outcome | Remaining work |
|---|---|---|
| G1 overlapping CoworkerMemoryPolicy, MemoryCandidate and learning events | Corrected at COWORKERS-TARGET; LEARNING/DATA-MODEL are single candidate/auto-commit owner | promote those typed proposals only through machine schema/migrations |
| G2 ordinary vs Coworker Conversation selection precedence omitted from API/FLOWS/SERVICES | Explicit present-day vs accepted target branches added | serve immutable Conversation owner and native selection |
| G3 Task may accept forged/conflicting coworker_id from Coworker-owned source chat | Server-derived origin and reviewed handoff guard specified | real Task source validation and negative tests |
| G4 quick Coworker Create and first chat described as falsely atomic | Made two independent retry-safe commits with separate idempotency keys | UI recovery and source owner API |
| G5 native chief typed responsibility proposal missing authority bridge | Defined qualified tool or inert user-reviewed editor; no planner | proposal source digest, draft service and request schema |
| G6 manual Add Memory targets Coworker/Personal while current UI upload supports Workspace notes | Separated authored ContextDocument from LearningCandidate; UI must show unsupported scope | user/Coworker Resource owner upload, authorization, UX |
| G7 default private learning vs legacy require-user-confirmation field | Normalized to LEARNING policy + informed AUTO_CONFIRMED_PRIVATE default, opt-out | persistence and actual classifier/conformance |
| G8 double counted gateway/native telemetry without verifiable join | Unreconciled overlaps excluded from exact total and labeled partial | provider/native identity receipts and scoped accounting migration |
| G9 opaque subscription implies enforceable hard monetary cap | Disallowed hard-currency guarantee without qualified billing/enforcement | operation/time budget and cost coverage UI |
| G10 ProviderCircuitService vs gateway/model routing health scope | Constrained Core circuit to Core-mediated capability provider invocation | source-specific outage classification tests |
| G11 terminal Take Control claimed for upstream native-private PTY | Limited control lease to mediated PTY only | real PTY/ConPTY/OS and native harness conformance |
| G12 model/provider choice hidden in old Conversation flow | Corrected to current native agent/model composer affordance | actual native UI and USER tests |
| G13 massive overlapping docs and repetitious generated reviewer material | canonical owner precedence retained; exact repeated prose examined: mostly epics' per-story acceptance gates, not duplicate domain stores | don't delete test gates to cosmetically reduce line count |
| G14 all proposed new telemetry/terminal/browser/Coworker routes | Target only, correctly not added to current OpenAPI by prose | explicit owner story with event/DTO/DB, CODE/SYSTEM/USER tests |

## Release correctness criteria

1. Per user-visible control prove current canonical Operator method or documented qualified native action, real load/error/permission/unknown states and test.
2. Every new persisted domain type or transition must be promoted together in owning API/SCHEMAS/EVENTS/STORAGE + migration and persisted idempotency receipt; no shadow DB model from a V1 UI LLD.
3. Native-agent behavior is verified against exact supported agent and version, not deduced from ACP, catalog metadata or provider proxy; unmediated actions remain unobserved.
4. Cross-owner request dry-run must reach an evidence-backed outcome through UI -> service -> durable receipt/outbox -> native/provider -> reconciliation -> verification -> UI.
5. A failed real-world dependency must remain honest (UNKNOWN/UNAVAILABLE/AMBIGUOUS/PENDING), with source/time and bounded repair. No false green check from script/lint coverage.

This audit covers high-risk semantic owners and generated line-by-line retrieval evidence; it does not mathematically prove absence of every prose contradiction in a changing ~55k-line corpus.
