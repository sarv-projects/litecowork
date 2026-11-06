[Reading 327 lines from start (total: 327 lines, 0 remaining)]

[Reading 291 lines from start (total: 291 lines, 0 remaining)]

[Reading 219 lines from start (total: 219 lines, 0 remaining)]


# V1 desktop screen and interaction contract

**Authority:** this document defines the V1 target screen structure and interaction semantics. It is subordinate to ARCHITECTURE.md, PRODUCT.md, V1-PRODUCT-RELEASE-CONTRACT.md and the owning domain contracts. It is a behavioral contract, not a screenshot-only design.

**Implementation evidence:** the current source state is mapped in IMPLEMENTATION-STATUS.md. A screen may exist while one or more actions are unavailable, local-only, preview-only or not qualified. The UI must render those distinctions honestly.

## 1. Product shell and navigation

LiteCowork is one desktop workspace, not separate Chat, Code and Cowork applications. The shell has a persistent left navigation rail, a central working surface and a contextual right Workbench. The selected Workspace scopes conversations, Coworkers, resources, Tasks, routines, Automations, artifacts and permissions.

The normal entry point is New Chat. Ordinary questions do not force Task creation, Coworker setup, a workflow wizard or capability installation. New Task is an explicit route/action for durable outcome-oriented work.

Primary navigation:

| Surface | Purpose | Required shell behavior |
|---|---|---|
| New Chat | Start an ordinary Conversation | Prominent, available when Workspace and local Operator prerequisites are satisfied |
| Recent Chats | Find and return to Conversations | Search/filter, pagination, durable state, draft/stream separation |
| Coworkers | Persistent named assistants | Roster, create, edit, identity, scoped context and authorized responsibilities |
| Needs You | Human attention inbox | Real count only; approval, UserRequest, sign-in, conflict, takeover and recovery actions |
| Work | Durable Tasks | Outcome, accepted plan, steps, attempts, events, blockers, artifacts and evidence |
| Automations | Reusable Routines and scheduled triggers | Preview, revision, host requirements, occurrence history, misfires and enable/pause |
| Library | Resources and finished Artifacts | Provenance, exact revisions, qualified preview/edit/export/compare capabilities |
| Discover | Capabilities and marketplace | Source, host compatibility, installation state, Connections and grants shown separately |
| Settings | Runtime, Workspace, agents, privacy and preferences | Every operational setting has a persisted owner or is visibly a local UI preference |

Goals, Notifications and Suggestions may appear as secondary Workspace surfaces. They must use their own owner contracts and must not crowd the primary navigation or imply unavailable delivery/execution capabilities.

### Default layout and responsive behavior

On desktop widths, the Workbench is visible by default in a compact state beside the central surface. The user can collapse, resize or expand it. The visibility preference may be stored per Workspace, but stored layout must not persist authorization, stale browser/agent handles or grants. At narrower widths, the Workbench becomes a focus-managed drawer and can be opened on demand. Navigation and Workbench drawers restore focus correctly and honor Escape/Tab behavior.

The default Workbench start surface offers entry points only. It must label each provider as available, needs setup, unsupported or unavailable from a real provider-health observation. A tile is not evidence that a real browser, terminal or computer provider exists.

## 2. Conversation and composer

The Conversation is the default surface and the authoritative human-visible history. It supports normal text, supported attachments, selected native agent context, durable message history, streaming/reconnection, stop, retry and recovery.

### Composer behavior

1. User selects a qualified AgentBinding. Agent-specific model, effort, mode, commands, mentions and references are shown only when the selected AgentModule declares them for the exact installed version and platform.
2. The draft remains local until submission. Switching the selected agent changes the binding used by a future turn; it does not transfer a private session or grant permission.
3. Send submits a typed Conversation turn with a stable request identity and the exact selected binding. A committed user message and an OPEN turn are not represented as a completed answer.
4. The UI shows progress only from correlated runtime/provider events. A stream gap triggers a persisted projection refresh.
5. Stop requests cancellation and waits for authoritative settlement. A local button click is not proof the process stopped.
6. Retry is available only after the prior turn's terminal or reconciled state permits it. Ambiguous dispatch locks duplicate submission until reconciliation.
7. Errors preserve the user's draft and display typed recovery guidance. A transport error after durable admission is an ambiguous result, not an invitation to submit a new request identity.

A Conversation does not create a Task unless the user explicitly promotes outcome-oriented work through a reviewed action. A Coworker is optional. Conversation history can outlive a provider process without claiming the private provider session survived.

### Current implementation boundary

The current UI has persisted cursor-paginated Recent Chats (including parent-sidebar Load more), create with an idempotency journal, snapshot/message pagination, local drafts, agent-binding selection and an OpenCode-only submit path. Send remains visibly enabled despite the target disabled-until-qualified gate and does not require a qualified binding/provider or Operator readiness. The visible Stop control is not effective for the OpenCode path: it searches presentation-stream transient turns that the native path does not emit, and its fallback request uses a hard-coded version. The cancel Operator route immediately marks the turn CANCELLED without interrupt/quiescence. Failed-turn Retry has an Operator route but no UI call site; the separate pending-create journal retry is available in the New Chat error state. The composer does not consistently fence OPEN/WAITING_DEPENDENCY/CANCEL_REQUESTED, and the submit transaction lacks strict single-flight admission. Add context/file and Voice Input are inert, More options has no handler, Turbo is static, @/slash copy is placeholder and every submit sends resource_refs=[]. The empty state contains hard-coded Antigravity branding/capability claims. The active-turn projection has no aggregate version. See Feature 001 SPEC and its UI behavior crosswalk for exact source details. Other provider selections are not proof of a supported Send route; do not infer agent support from a profile probe or catalog entry.

## 3. Work and Task detail

Work is the durable execution workspace. A Task is distinct from a Conversation and is created only through explicit promotion or an authorized Automation occurrence.

The target Task screen contains:

- **Overview:** objective, Workspace, origin Conversation/Coworker revision, lifecycle state, active runtime, placement and blockers.
- **Plan:** immutable accepted PlanRevision, ordered Steps, dependency edges, acceptance criteria and supersession history.
- **Activity:** durable event timeline with actor, Attempt, timestamps, state transitions and correlation.
- **Workers:** lead and delegated workers, exact AgentModule/version, eligibility, scope, lease, budget, outcome and verification.
- **Files and Artifacts:** input pins, actual revision changes, committed outputs and provenance.
- **Inspector:** technical traces, tools, process/environment identity, policy decisions, receipts, tests and redacted logs.
- **Controls:** pause, resume, steer, cancel, retry or recover only when the owning service admits that exact transition.

### Mission Control and visual execution

The multi-agent execution view belongs inside Work/Task detail, principally Plan, Activity and Workers. It is a visual projection of durable Steps, dependencies, Attempts, AgentSessions, Effects, Artifacts and Evidence. It is not a free-standing animated graph or a second workflow engine.

A node may display running, blocked, failed, completed or independently verified only when the owner record supports that state. Every node opens its actual inputs, outputs and evidence. Do not invent percent complete, ETA, elapsed-time predictions, agent presence, worker count or progress animations. An ordinary user sees outcome, steps, responsible worker, result and blocker; a technical user can expand the Inspector.

### Current implementation boundary

The current Work screen lists persisted Tasks, opens Task details, supports objective/specification revision and requests planning readiness. The readiness result explicitly states that planning and dispatch are unavailable. The current UI does not have an executing Plan/Attempt timeline or operational Task controls. A READY Task record is not a running Task.

## 4. Needs You

Needs You is an actionable inbox, not a decorative count or general activity feed. It groups persisted approvals, UserRequests, authentication actions, conflicts, human takeovers and recovery decisions.

Each item identifies the exact Workspace, Task/Conversation/Coworker, requester, requested action, scope, expiry, consequences and current state. A user response binds to the exact request/operation/version. An approval is never represented as a free-floating "yes"; it must be bound to the action, target, plan/effect digest, scope, expiry and owner identity.

Items show loading, stale, empty, unavailable, resolved and error states distinctly. The UI refreshes from owner projections after an action; optimistic styling alone is not proof of resolution.

## 5. Coworkers

Coworkers are optional persistent assistant identities, not always-running processes and not Tasks. A Coworker definition can include name, purpose, instructions, selected chief AgentBinding, eligible worker profiles, scoped memory/knowledge, Connections and approved responsibilities/routines.

The roster shows identity, status and recent persisted activity. Detail areas are progressively disclosed:

- Overview and instructions.
- Chats/conversations associated with the Coworker.
- Memory and knowledge with scope, source, provenance, eligibility, revision and owner controls.
- Agent and worker assignments.
- Connections and their authentication health.
- Routines/responsibilities and approval state.
- Activity and revision history.

Editing Coworker instructions does not authorize a capability, add a Connection, install a package, enable an Automation or grant a worker. Memory proposals and auto-learning obey the canonical LEARNING.md policy; chat text must never silently become authority.

### Current implementation boundary

Coworker configuration, list/detail, revisions, status and Workspace-primary operations are persisted; the presence route is mounted but currently returns 503 DEPENDENCY_UNAVAILABLE. CoworkerSettings is a roster/settings editor, not the target chat-first surface. It persists name/role, default lead, delegation strategy, worker-profile allowlist, interaction defaults, lead-failover preferences, context policy and notification preferences. Only default lead is consumed by Task creation; lead-failover is copied into TaskSpec without a production fallback dispatcher; interaction/notification/delegation strategy have no production consumer; the worker-profile allowlist/budget are read by a SQLite child-Attempt admission primitive with no daemon/Operator caller. Avatar selection, delegation-budget editing and delegation-policy-source selection are not exposed. Coworker PAUSED is only partially enforced: Automation enable checks Coworker active at enable time, but existing enabled Automations are not re-fenced at occurrence settlement; Task storage rejects ARCHIVED, not PAUSED. The Task composer nevertheless rejects PAUSED Coworkers for owner Tasks, contrary to the target contract. Quick Create currently creates an Assistant and optionally sets primary but does not open a Coworker chat. Conversation records have no Coworker origin/revision. CoworkerContextPolicy is persisted but retrieval does not enforce its allowed kinds, item limit or Task-summary setting. Coworker Settings lists COWORKER_NOTES and GOAL_NOTES although the current ContextDocument create validator rejects those owner kinds; no Coworker Memory browser, Learning Inbox or private-memory revoke control is served. Those states remain unavailable until their owners are wired and qualified.

## 6. Automations and Routines

A Routine is a reusable work definition. An Automation is a trigger/schedule attached to a revision and an eligible local TriggerHost. Saving a Routine does not schedule it. Enabling an Automation is an explicit, reviewed state change.

The flow is: choose or create a Routine → review the exact inputs and outputs → select a trigger or schedule → set timezone and DST policy → inspect host and dependency requirements → preview the next occurrences and overlap/misfire policy → review grants and budget → explicitly enable.

The UI shows each occurrence with its exact identity, source revision, TriggerHost Runtime/incarnation, due time, decision, materialized Task or typed blocker, and settlement. "Run once" and "Test run" are explicit manual actions and do not silently change recurring enabled state.

Local scheduling requires an awake machine and an available Runtime/TriggerHost. A missed local occurrence is not described as running. Laptop-off continuation requires a separate cloud Runtime and is outside local V1.

### Current implementation boundary

OperatorServer starts the local AutomationTriggerHostWorker, which polls persisted cursors every 30 seconds and settles supported recurring schedules. Manual Run creates an occurrence plus READY Task; Create test Task creates READY Task without occurrence/cursor. Neither starts Task planning or agent execution. Definition list pagination has Load more, but Automation occurrence history consumes only its first API page and drops next_cursor. The Pause success message incorrectly says trigger execution is unavailable in this build; the worker is active. retry_policy and wake_policy are saved but have no production consumer. See Feature 010 SPEC/RESEARCH for the source crosswalk.

### Notifications current boundary

The Notifications surface has cursor-paginated delivery history and versioned Workspace event-class preferences. Outcome settlement inserts PENDING delivery rows; no production sender/consumer advances them to SENDING/SENT/FAILED/AMBIGUOUS/SUPPRESSED. PENDING is not delivered. The free-text preferred_channels values are persisted but ignored by the producer; ON_CONDITION means only DEPENDENCY_BLOCKED and has no named-condition editor. Quiet hours are displayed and preserved on save, but cannot be edited. Delivery history does not prove a desktop, email or provider transport receipt.

## 7. Discover, LiteSPM and Connections

Discover separates four facts:

1. **Explore:** catalogue/listing information with source, provenance, installability level and verification status.
2. **Installed / native:** observed host inventory, including freshness and incomplete-inventory caveats.
3. **Managed:** packages that LiteSPM actually owns, with plan/apply/receipt/reconcile lifecycle.
4. **Connections:** authenticated accounts/providers and scope health.

For every capability, show package kind, source, version/revision, target hosts, OS support, compatibility, installability, required secrets, launch contract and verification provenance. Discovery-only listings are never represented as installable.

Install/adopt/update/repair/unmanage/remove preview the exact plan, target, expected configuration mutation and rollback. LiteSPM is the only package lifecycle/configuration writer. An adopted native entry remains user-owned when unmanaged. LiteCowork separately manages Coworker assignment, Conversation/Task grants and invocation authority.

A skill, MCP server or plugin is not usable merely because it is listed or installed. If an action does not have a qualified owner operation, the UI explains why it is disabled.

### Current implementation boundary

The current Discover screen reads Workspace-local skill records and bounded Runtime-observed hosts and can open a review-only capability disclosure. All Discover row actions are disabled; connect-or-install returns 503 and capability test returns UNKNOWN without provider contact. Needs You ApprovalDetail is a real exact-digest decision surface, but APPROVE records only the owner decision: it does not create a reusable grant, dispatch an Invocation or execute an external Effect. LiteSPM management and account Connections/OAuth are not integrated/qualified. No Install, Adopt, Connect, Use or provider-write action may imply success in this build.

## 8. Library and Artifacts

The Library includes Workspace resources and committed Artifacts. Each item records source, exact revision, owner, Workspace, origin Conversation/Task, MIME/format, content hash, timestamps and provenance.

Each format declares its actual capabilities: read-only preview, edit, save, export, compare, render or unsupported. A preview is not an editor; a text extraction is not faithful layout rendering; PDF display is not Word editing; and a generated file is not an accepted deliverable until its required checks pass.

Editing uses a draft revision and an explicit save/commit operation with conflict handling. Restore and delete honor owner/scope fences and retention policies. Opening an Artifact after restart resolves the committed version, not a stale WebView buffer.

### Current Library / Resource surface

The mounted Library provides bounded multi-file upload/quick-import (up to 100 files / 100 MiB per selection), saved Resource list and pagination, metadata search, indexed local-text search, bounded on-demand text scan, exact-revision preview, source-reference preview, Save original, text-index rebuild, Resource revision history/edit, and Workspace Note creation. It can pin selected exact Resource revisions as Task inputs, but the UI says these are not read by an agent until Task execution exists. Search hits are not silently attached to Conversation. ZIP files are saved intact; the mounted ZipIntakeNotice reports extraction unavailable and contents remain opaque, unindexed and unattached. PDF/Office/image originals may be stored, but the current local text index/preview does not extract them. A source ZIP metadata-preview route is not called by the UI and its isolated worker is development-feature-only, absent from release sidecars.

WorkspaceRoot controls are colocated in the Library under Persistent folders. Add uses the native picker and immediately commits on supported Linux/macOS without a separate confirm; Windows Add remains enabled but the daemon returns 503. Scan is a Linux-only metadata census; Windows/macOS return 501, after which the UI disables further attempts. Observed-file browsing shows names/size/freshness and does not imply content revisions. Root scan does not ingest, watch or index files. Grounded retrieval is a separate read-only lexical endpoint; no ContextAttachment, Context Used panel or Conversation retrieval wiring exists. See Feature 003 SPEC/RESEARCH and IMPLEMENTATION-STATUS for the exact source crosswalk.

### Current Artifact boundary

The Library's Saved, Task outputs (TRANSIENT) and Archived views list already committed Artifact projections; there is no create/generate/import-convert action or served Artifact-create route. ArtifactWorkbench reads exact immutable versions and provenance, previews selected text formats, compares exact stored versions, and permits bounded managed `text/plain` append/edit only. Markdown/CSV/JSON/XML/YAML preview does not mean those formats can be edited or round-tripped. Office/PDF uploads remain Resources, not Artifacts. Task outputs are read-only and have no live producer while Feature 005 Task execution is unavailable. Native Save As writes one exact managed blob version locally (up to 10 MiB) and creates no Artifact/Resource revision, domain event or ExportReceipt. See Feature 007 SPEC/RESEARCH and `docs/IMPLEMENTATION-STATUS.md`.

## 9. Workbench

The Workbench is a contextual panel over actual authorized objects. It is not a set of mock demos.

| View | Required behavior | Current boundary |
|---|---|---|
| Start | Surface available providers and entry points with truthful health | Launcher tiles do not imply service availability |
| Files | Workspace resource tree/details and exact-revision preview; edits use a qualified mutation path | Current list/preview is not a general filesystem editor |
| Browser | Real browser Environment, profile/session identity, navigation, screenshots, DOM/network capabilities, policy, takeover and return | Current iframe is only a WebView preview; not Chrome/CDP automation |
| Computer | Real screen/control provider with lease, OS capabilities, consent and human takeover | No qualified computer-use provider is connected |
| Terminal | Real daemon-owned PTY/ConPTY, process identity, stream, resize, bounded scrollback, detach/reattach, stop and recovery | Current terminal is simulated and must not report a command exit or Job Object containment |
| Diffs | Actual VCS or revision-to-revision diff with file identity, hunks and provenance | Current sample/generated diffs are not actual changes |
| Activity | Persisted events, provider reports and verified records with assurance level | Do not render synthetic token/process traces |
| Artifact preview/edit | Format-specific renderer/editor and revision commit | Only enable features proven by that Artifact handler |
| Inspector | Redacted execution, authorization and recovery detail | No secrets, native handles or unverified state |

A terminal session belongs to the daemon Environment, not the React component. Switching tabs detaches the view; it does not kill the process. Runtime restart marks an old PTY lost and never silently reruns the command. Bounded output retention, backpressure and explicit termination are mandatory.

A Browser iframe preview must not be labeled a sandboxed agent browser or used as evidence of browser control. A true browser controller has its own OS process/profile, egress policy, session identity and takeover contract.

### Current Workbench source boundary

The Files view is a Resource projection/preview, not a live filesystem browser. Browser is a sandboxed iframe with local React URL state, not Chrome/CDP and not a managed browser session. Terminal is React-local simulation: no process spawn or Tauri IPC; arbitrary commands print a fabricated exit code 0 while the UI claims pwsh/ConPTY and Job Object containment without a process receipt. Diffs use a fixed modulo pattern or static sample patch with hard-coded counts, not VCS or exact revision comparison. `WorkbenchTab` includes an Activity type but no navigation or render branch; it falls through to Diffs. Computer-use has no capture/input provider or live UI. The terminal's fake-success/containment claims are a visible truthfulness defect to remove, not a feature to qualify by adding a backend later. See Feature 008 SPEC/RESEARCH and `docs/IMPLEMENTATION-STATUS.md`.

## 10. Intelligent UI and presentation

Every answer keeps a durable semantic message. Presentation enhances that answer; it does not replace semantic truth.

The target presentation layers are:

1. Semantic response: ordinary text/Markdown and committed conversation message.
2. Rich presentation: typed cards, callouts, tables and structured summaries.
3. Generated media: MIME-validated image/media Artifacts with hash and provenance.
4. Intelligent UI: schema-validated declarative tables, charts, calculators and structured interfaces using registered components and restricted state reducers.
5. Interactive Artifact: a self-contained application executed only inside the separately qualified isolated Artifact Runtime.

The preferred agent interface is a typed structured output/tool event. A text fence may be accepted only as untrusted input that passes the same schema and size/depth validation. Reject unknown components, arbitrary script, network imports, host APIs, event handlers or hidden authority. Bind the presentation to the committed semantic message and content digest. The renderer may use only registered component types and bounded local state. Provide accessible text/table fallback.

A calculator evaluates a restricted arithmetic AST, not eval. A chart offers an accessible data table. Rendering failure never removes the semantic answer. A CSP alone is not an isolation boundary for an executable Artifact.

### Current implementation boundary

Rich-presentation routes and renderer code exist. This does not qualify the complete five-layer stack, every structured component, generated media, an arbitrary interactive application or all output formats. Intelligent UI acceptance requires schema tests, renderer behavior, fallback, accessibility, size/depth limits and malicious-input tests.

## 11. Settings

The normative V1 Settings information architecture is defined once in §14 below: 16 grouped categories, progressively disclosed according to owner readiness. Do not maintain a second, competing category taxonomy here. The 187-control per-action target and failure contract lives in [V1-SETTINGS-CONTRACT-LEDGER.md](V1-SETTINGS-CONTRACT-LEDGER.md).

Every control is one of:

- Persisted and verified: the owning service returned the committed state.
- Local UI preference: actually applied and stored locally, with presentation-only scope.
- Read-only observation: shows source, freshness and scope.
- Unavailable: visibly disabled or omitted with the missing prerequisite.
- Pending/ambiguous: retains operation identity and reconciles before retry.
- Error: shows a typed repair path.

Never expose a local React state variable as a persisted security/terminal/browser setting. A local selector that changes only its own component state is not a functioning preference. Cache clearing must actually clear the selected browser provider's cache, not merely display a notice. Runtime status must distinguish process availability, authenticated Operator serving/readiness, persisted lifecycle availability and Task execution capability.

### Current source boundary (2026-11-06)

The active App.tsx Settings route mounts `SettingsView`, which exposes four categories only: General, Workspaces, Agents and Runtime. The 16 categories in §14 are target IA, not current navigation. General appearance values are ephemeral component state, not persisted or applied; Ctrl+N/Ctrl+W/Ctrl+K are display-only shortcut labels with no matching active key handlers. Workspaces exposes selection/refresh/create, instruction revision editing and archive, but not WorkspaceRoot controls, rename, policy mutation or Runtime enrollment. Agents exposes bounded catalogue/profile-probe/binding operations. Runtime exposes start/refresh and lifecycle observation; the visible card does not offer stop/drain or startup-policy mutation. The exported `SettingsPage` function in App.tsx is unmounted. See [SETTINGS-AND-SHELL-SURFACE-AUDIT.md](SETTINGS-AND-SHELL-SURFACE-AUDIT.md) for the complete control crosswalk. The Runtime UI's “Connected (Localhost)” label is inconsistent with the actual OS-local socket/named-pipe transport and must not be repeated as a transport fact.

## 12. Global interaction and accessibility rules

- Every actionable control has a real command, defined disabled state, loading state, success receipt, error and recovery.
- No button is enabled solely because a component or enum exists.
- No optimistic state is authoritative for approval, installation, execution, effects or task completion.
- Preserve drafts on failure. Bind retries to the original idempotency identity when the owner says the outcome may be ambiguous.
- Use source and freshness labels for observed capabilities; show unknown distinctly from empty.
- Respect keyboard navigation, focus visibility, screen readers, reduced motion, contrast and non-color status.
- Motion illustrates real state transitions and must stop for reduced-motion preference. Do not animate invented worker activity or fake progress.
- Keep the default surface plain-language and progressively disclose model/version, policy, process, lease and trace detail.



[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]
[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]## 14. Settings, Agent profile and Usage — V1 release surface

This section resolves the screen-level IA. Detailed individual controls, mutation class, owner and failure contracts remain in [V1-SETTINGS-CONTRACT-LEDGER.md](V1-SETTINGS-CONTRACT-LEDGER.md). Feature-specific Agent lifecycle behavior remains in [Feature 002](../specs/002-native-agent-install-auth-binding/SPEC.md). Usage accounting remains owned by [V1 Telemetry/Usage/Resilience LLD](V1-TELEMETRY-USAGE-RESILIENCE-LLD.md). These are normative targets; served behavior is still determined by IMPLEMENTATION-STATUS.md and the active route inventory.

### Settings shell and navigation

Settings uses a persistent category/navigation column beside a focused main panel. The column contains a small search field and grouped categories; selecting a category changes only the main panel, not global application navigation. On narrow windows the category column becomes a focus-managed drawer, and Back returns to the prior category and query. Deep links preserve selected Workspace/Runtime/profile scope but never restore grants or stale authorization.

V1 category order:
1. **General** — Appearance, language, density, text scale, reduced motion, shortcuts. Local presentation preferences only.
2. **Notifications** — Delivery preference, sound preference, quiet hours and preview. Delivery channel must be backed by an OS integration; do not show a delivered notification from a local preview.
3. **Workspaces** — Workspace selection, create/rename/archive, roots, instructions and default lead. Actions use the Workspace owner and exact version.
4. **Agents** — Catalog, Installed, Profiles, Workspace bindings/defaults and Usage shortcut. No generic Providers tab.
5. **Connections & Tools** — Discover/LiteSPM package links and connected accounts only to the degree each LiteSPM/Connection handler is served.
6. **Local Models** — runtime-scoped detection and compatibility only; never a global provider router.
7. **Coworkers & Subagents** — Coworker defaults, delegation policy and worker profiles. Must disclose that settings affect future Task admissions unless a specific owner contract says otherwise.
8. **Runtime** — daemon availability, readiness, startup mode, recovery and health, each with exact lifecycle semantics.
9. **Terminal** — shell profiles, scrollback, retention and paste/integration preferences only when the daemon-owned PTY service exists. Viewport-only preferences may remain local.
10. **Browser & Computer** — only qualified profile, permission, session and control surfaces. No active settings while providers are unavailable.
11. **Memory & Knowledge** — policy and source management only after the memory/context owner operations are available.
12. **Security & Privacy** — grants, credential presence, egress, retention and revocation using Trust/security owners.
13. **Storage & Backup** — storage report, cleanup preview, backup/export/import/restore with explicit preview and receipt.
14. **Usage & Monitoring** — read-only usage, reliability, source coverage, quota and health; filters local and do not mutate authority.
15. **Advanced** — OS power assertion/keep-awake, startup/recovery policy, diagnostics and opt-in local debug behavior. These are separated into OS action vs Runtime policy.
16. **About & Diagnostics** — version/build info, redacted logs, provider diagnostic, support docs and previewed support bundle.

Do not add visible tabs merely because the target list contains a possible operation. If the owner operation is not served, omit the control or show a non-interactive explanation with the exact prerequisite. No inert clickable row.

### Notifications panel

The preference model separates sound from OS notification delivery: Sound + notification, Sound only, Notification only, Neither. Quiet hours and in-focus suppression apply only to supported notification classes and are displayed with timezone. "Try it" is a device-local preview and reports whether the sound preview was played; it must not claim an OS notification was delivered unless the OS notification provider acknowledged it. Notification queue and Needs You count are different concepts; the former is delivery, the latter is actionable human attention.

### Advanced panel

Keep-awake choices: Off; While agents/work run (also on battery); While agents/work run on power only. These must map to OS power assertions with independent battery/power behavior and be labelled unavailable on unsupported OS. Resume after restart is not a generic switch for all native processes: it governs only LiteCowork-owned durable admitted work whose recovery contract is qualified. It does not restart arbitrary third-party CLI sessions or recreate hidden process state. Show runtime start mode separately from resume policy. Any change affecting an active Runtime takes effect at the documented boundary and reports pending/restart-required honestly.

### Agent profile screen

The agent catalogue and one selected Agent profile are different levels. Catalog entry shows reviewed logo/fallback, vendor and purpose, source/license, OS/Runtime compatibility, independent detection/install/config/auth/readiness state, protocol/capabilities, version and actual last-used time if observed. Details opens a profile with Overview, Installation, Authentication, Capabilities, Workspace bindings and Diagnostics. The exact action set follows Feature 002. Multiple installations or profiles for the same agent on Windows, Linux or WSL remain distinct Runtime-bound observations. No global Provider selector or LiteCowork-managed native model router.

### Agents work overview

Do not create permanent sidebar routes for Needs You, Working and Done under Agents. Provide an Agents/Work overview or Work page filter strip with three summary tiles:
- **Needs You** — Tasks requiring user action from real UserRequest/approval/blocker projections; route opens the matching Needs You item.
- **Working** — Tasks with at least one admitted, nonterminal Attempt/Step. A READY Task or OPEN Conversation is not Working.
- **Done** — terminal Tasks whose acceptance/verification state is shown distinctly; completed is not necessarily verified.

Tiles are clickable filters over the same Work/Task store used by Work. Counts use the selected Workspace and are refreshed from owner projections. Cards are grouped by Workspace/project only if the group is backed by a real owner relation. A card shows objective/title, state, owner Coworker/lead, exact agent binding and model only when known, timestamps/elapsed duration from actual persisted boundaries, latest meaningful result, blocker/attention and real cost/usage only when telemetry coverage supports it. Context menu actions are owner-admitted actions, not generic ellipsis decoration. The page must not create duplicate Tasks or duplicate counts alongside Work and Needs You.

### Usage and monitoring dashboard

Usage is a first-class Settings destination and can be opened from an Agent profile with the corresponding AgentBinding filter selected. The default top-level view is scoped to current Workspace; switching to All Workspaces requires an explicit scope selector and authorization. The dashboard has quick ranges Today, Last 7 days, Last 1 month, Last 3 months, Last 1 year and All time. Expanded date options include YTD and Custom. Storage uses UTC; day boundaries and display follow the selected user's timezone; intervals are half-open [start,end).

Primary filters: agent/harness/AgentBinding, model, provider, Coworker, Conversation/session, Task, Attempt/PlanStep, lead/subagent, Runtime, source/confidence and outcome. Filters compose without duplicate leaf counts. Grouping: Day, Session, Coworker, Task, Agent, Model, Provider. Keep filter state in the current view and optionally remember grouping per Workspace; never treat filters as permissions.

Overview headline metrics: deduplicated inference requests; input tokens; output tokens; cache read/write tokens where reported; billed amount by currency; estimated equivalent list price in a separate lane; coverage percentage and UNKNOWN count. Secondary charts: requests/tokens over time, cost by model/agent where price basis exists, latency/failure rate, quota windows and reset times where the harness reports them, cache ratios when source reports counters. Provide an accessible underlying table for every chart.

Usage detail table columns: grouping label, requests, input/output/cached tokens, estimated equivalent cost (marked est.), provider-billed amount (only when provider receipt supports it), subscription/quota observation, completeness/coverage, source/confidence and time range. Selecting any row opens leaf inference observations; selecting a leaf opens provenance (adapter, verified response identity, model, price snapshot, confidence, timestamps, outcome), never prompt or response content. Export is explicit, scoped to the current filter/window, redacted and bounded.

Accounting rules: one immutable leaf per actual inference; deduplicate native/gateway/provider observations only by verified request identity; count real retries that sent another inference separately; do not count transport replays twice; rollups are derived from leaves only; do not add lead aggregate plus child leaves; split group allocation only when the attribution method is defensible; currencies never combine implicitly. Cache and reasoning subtotals must not be double-added to their parent input/output counters. Missing counters are UNKNOWN, not zero. Native subscription included usage is quota/entitlement, not a $0 API bill. Estimated list price is not provider billing. Report model/provider price source and effective snapshot date.

When data is missing, say exactly which agent/harness does not expose it. Show no measured usage yet when there are no accepted usage observations; do not render $0.00, 0 tokens or 100% coverage as a default. A local session reader, when qualified, extracts only usage counters, model/response IDs and timestamps from the user's own agent files; it must never ingest prompts, completions, reasoning text, secrets or raw transcripts. Local reads are opt-in and scoped to a supported harness/Runtime. Opaque subscription usage remains UNKNOWN unless the provider exposes a supported usage source.

All Usage dashboard surfaces are read-only in V1. Refresh requests a scoped projection refresh; export produces a local redacted report. No price edit, quota reset, billing-account connect or "optimize spend" button is implied by the dashboard. Those require separate owner contracts.

### Global settings action rules

Every button, row, menu item, slider, select and segmented choice in Settings must be recorded in V1-SETTINGS-CONTRACT-LEDGER.md and map to one owner, exact scope, current status, enabled condition, command, pending/loading, success acknowledgement, error/denial and recovery. A local preference commits locally; native action returns a verified OS observation; Operator mutation returns a committed receipt/version; read-only observation displays source and age. On cancel/failure preserve the prior committed value. Do not optimistically display active state. For controls not qualified on V1 OS, remove them from the interactive layout rather than shipping a fake preview that looks like a working setting.

## 15. OpenBot comparison — compact Task, model picker and Workflows UI — 2026-11-06

Repository/source inspected: nightly-labs/openbot@6759fa3ee857f4f4966865e12de99ae84ab6de07 (version 0.62.0, 2026-11-06). See [pinned source study](OPENBOT-REFERENCE-IMPLEMENTATION-STUDY.md). This section captures reference patterns only; LiteCowork's owner contracts remain authoritative. OpenBot's README calls it a development preview and documents danger-full-access/approvalPolicy-never; its license is PolyForm Noncommercial 1.0.0. No source code or permission model is adopted.

### Main navigation and Tasks view

Keep the main application rail compact and icon-led, with labels/tooltips on hover/focus and no permanent sidebar item for every agent, model or node type. Work is the one durable work destination; Needs You is its actionable attention subset. Automations is for schedules/trigger management. An external Workflow opens in its provider's own editor through a reviewed provider-supplied link; LiteCowork keeps the Routine/workflow reference, input mapping, invocation controls and run observations in its own compact page, not a native graph canvas.

The Work/Task view has one composer, one compact Coworker/AgentBinding selector and grouped task list. Group headings show counts and sections only when nonempty. Opening a Task expands a right-side detail panel without leaving the list where window width allows; at narrow sizes it becomes a full-screen detail route with Back restoring selection and scroll position. Keep the panel close action distinct from Stop Task.

Suggested LiteCowork grouping (owner-state mapping, not OpenBot label copy): Needs You; Working; Waiting; Ready/Queued; Completed; Archived. Needs You is derived from UserRequest/approval/blocker owner projections; Working means admitted nonterminal Attempt/Step; Waiting means explicit dependency or external/user wait; Ready/Queued means admitted but not yet executing; Completed preserves verification result; Archived is a separate visibility filter. Never derive status from a UI spinner or merely an open Conversation.

Task composer agent picker chooses Coworker/AgentBinding. A local last-used choice may preselect. Model and effort are secondary options only after the binding is selected and only if the current descriptor supports them. Keep files/attachments and the prompt visible; microphone is optional only if a real voice backend is enabled. A short undo delay is optional and must be a durable cancellable admission only if TaskService supports it; do not simulate Undo in UI state.

### Agent model picker

The model picker shown in the first supplied OpenBot image belongs to an Agent profile/composer model control, not the Task agent selector. LiteCowork uses a compact anchored popover for the selected agent/profile: profile identity + runtime version, searchable provider model list, selected checkmark/default marker, then an optional effort row at the bottom. No provider rail when only one provider/profile applies. Multiple accounts require an explicit account selector, not multiple model rows mixed together. Model and effort options are exact descriptor metadata; no universal dropdown or assumed Low/Medium/High.

### Usage dashboard presentation

The supplied OpenBot usage image has a useful hierarchy: thin top filter row, summary/provider list at left, daily trend at right, compact totals strip, then a breakdown table with grouping toggle. LiteCowork can adopt this arrangement, responsive stacking and compactness. Data meaning remains governed by V1-TELEMETRY-USAGE-RESILIENCE-LLD: API-equivalent estimate is not bill, provider-billed actual is a distinct lane, quota is separate and unknown is never zero. Do not use provider cards with fake limits or model costs.

### Workflow integration: manual authoring and agent assistance

OpenBot demonstrates a visual graph editor whose saved graph can also be edited by agent tools, but LiteCowork's accepted architecture does **not** include a generic in-Core workflow graph engine. Do not add a LiteCowork canvas, node palette, graph database or graph-run state machine to V1.

Manual graph authoring occurs in the connected external WorkflowBackend's own editor (for example, a user-owned n8n/Activepieces instance where supported). LiteCowork's Automation/Routine view may discover an existing workflow, show provider identity/reference, inspect supported inputs/outputs, validate a mapping and invoke it as one bounded CapabilityInvocation inside the parent Task. A provider-editor link appears only when the adapter returns a reviewed safe URL; LiteCowork does not embed an external editor or imply that a graph is locally editable.

Agent assistance inside LiteCowork V1 is for the LiteCowork-owned Routine and invocation mapping: formulate the objective, map supplied values to the provider's input schema, explain validation errors and summarize provider receipts. Users may separately use an external provider's own AI builder/MCP authoring tools when enabled in that provider (for example, n8n Assistant/MCP or Activepieces Build by Asking/MCP); the resulting graph remains in the provider and is edited/published there. LiteCowork's documented WorkflowBackendAdapter is a target contract, not a production implementation in the scanned source tree, and its target operations are read/validate/invoke/observe/cancel—not create/update/publish. No LiteCowork agent may claim to have created or edited the provider graph.

A future direct LiteCowork-Coworker authoring extension may allow a LiteCowork agent to call provider graph-mutation APIs only when that backend exposes a qualified draft/create/update/validate/publish API. This is distinct from the user invoking the provider's own Assistant/MCP directly in that provider product. The external provider must own the graph, schema and execution. A LiteCowork-originated proposal must use a staged provider-side draft, exact version/digest, validation, effect/scope summary and explicit owner confirmation before publish. This is a future adapter decision, not an active V1 control.

The inspected OpenBot graph has typed Trigger, Prompt/Text, LLM, Agent, Condition, Approval and Output concepts, plus annotation kinds; its schema has no dedicated Code node. External providers differ: n8n documents a JavaScript/Python Code node; Activepieces exposes JavaScript/TypeScript CODE steps through its builder/MCP; Node-RED offers a JavaScript Function node. n8n and Activepieces also document provider-native AI builder paths that leave the result as a provider-owned graph. Such code and AI authoring remain inside the external backend's own identity, credentials, host and isolation boundary. They must not be described as LiteCowork Environment processes or as automatically mediated by LiteCowork Trust. The current LiteCowork source has no production WorkflowBackendAdapter or WorkflowAuthoringAdapter, so none of these external provider authoring tools is currently exposed through LiteCowork.

### Visual density and responsiveness

Use a small rail, restrained single-line rows, subtle dividers, short titles and compact metadata. Do not repeat status chips that say the same thing at multiple levels. Keep one strong selected state, avoid extra colored badges, and use a neutral status hierarchy with one exceptional attention state. Main view stays uncluttered; deep configuration belongs in a selected-row/popover/side panel. At narrow widths, preserve primary content and move detail/inspector controls into a drawer.

## 16. Screen release acceptance

A screen is release-qualified only when every advertised action maps to an owner operation, authorization and durable state transition, and the screen has loading, empty, offline, stale, denied, failure, ambiguous and recovery behavior. The exact agent/provider/OS/build combination is recorded in ACCEPTANCE-AND-E2E-PLAN.md.

The implementation evidence for the current checkout is maintained in IMPLEMENTATION-STATUS.md. Any mismatch found in code is a release blocker or a deliberate target gap; it must not be hidden by a dated addendum or a success-style placeholder.