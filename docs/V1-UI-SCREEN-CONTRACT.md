


# V1 desktop screen and interaction contract

**Authority:** this document defines the V1 target screen structure and interaction semantics. It is subordinate to ARCHITECTURE.md, PRODUCT.md, V1-PRODUCT-RELEASE-CONTRACT.md and the owning domain contracts. It is a behavioral contract, not a screenshot-only design.

**Implementation evidence:** the current source state is mapped in IMPLEMENTATION-STATUS.md. A screen may exist while one or more actions are unavailable, local-only, preview-only or not qualified. The UI must render those distinctions honestly.

## 1. Product shell and navigation

LiteCowork is one desktop workspace, not separate Chat, Code and Cowork applications. The shell has a compact left navigation, a central Conversation surface and a contextual right object workspace. The selected Workspace scopes conversations, Coworkers, resources, Tasks, routines, Automations, artifacts and permissions.

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
| More > Discover | Capabilities and marketplace | Secondary entry; source, compatibility, installation, Connections and grants remain distinct |
| Settings | Runtime, Workspace, agents, privacy and preferences | Every operational setting has a persisted owner or is visibly a local UI preference |

The accepted persistent workspace destinations are Work, Coworkers, Library and Automations. Needs You is a compact attention entry; Settings is in the footer. A collapsed More disclosure contains Discover, explicit New Task, Dashboard, Goals, Notifications and Ideas; Routines is reached inside Automations. The table above is a route/surface catalog, not an instruction to expose all entries permanently. The 2026-11-14 revision proposes New Chat, Projects and unified Recents as frequent entry points with domain destinations grouped under More; this proposal is recorded in [Workspace Experience Revision](WORKSPACE-EXPERIENCE-REVISION-2026-11.md) and does not replace the accepted baseline until owner review. Goals, Notifications and Suggestions remain secondary Workspace surfaces. They must use their own owner contracts and must not crowd the primary navigation or imply unavailable delivery/execution capabilities.

**Current code under USER-DELEGATED A-123 direction (Feature 011-T19 PARTIAL):** the left rail now renders New Chat, Recent Chats, Projects, Needs You, More and footer Settings; secondary routes remain under More. Projects renders the existing Workspace list and creates no Project entity. Selecting a Workspace follows the current dirty-Library navigation guard and returns to a fresh blank Conversation without creating or sending a message. Recent Chats is still Conversation-only and shows explicit IDLE, LOADING, READY-empty/rows, FAILED and UNAVAILABLE states; no unified Recents feed or composite Needs You count is claimed. At widths up to 900px, the Conversation header now supplies a Menu trigger for the same modal navigation drawer, with an expanded state and shared focus-return ref; the hidden off-canvas sidebar is no longer the only navigation path. Focused shell, Conversation and source/component tests pass **56/56**, TypeScript and API type checks pass, and rendered browser groups passed shell reachability **4/4** and Conversation **15/15** in Chrome **158.0.8127.38** against the production-built harness on 2026-11-24. The shell checks cover Projects-to-blank-Conversation without a write/start/invoke side effect, More keyboard/current-route behavior, narrow drawer focus/Escape/route return, and Projects/New Chat through dirty-Library Stay/Discard. The Conversation checks cover truthful draft behavior, Runtime-unready send gating, transient stream/recovery, shell navigation and RichPresentation invalidation/retry. This is rendered browser CODE, not packaged Tauri, formal package acceptance, INTEGRATED or QUALIFIED evidence; Feature 010/014 and native platform gates remain. It does not change the accepted V1 target above or formal Feature 011 acceptance.

### Default layout and responsive behavior

On desktop widths, the Workbench is visible by default in a compact state beside the central surface. The user can collapse, resize or expand it. The visibility preference may be stored per Workspace, but stored layout must not persist authorization, stale browser/agent handles or grants. At narrower widths, the Workbench becomes a focus-managed drawer and can be opened on demand. Navigation and Workbench drawers restore focus correctly and honor Escape/Tab behavior.

The default Workbench start surface offers entry points only. It must label each provider as available, needs setup, unsupported or unavailable from a real provider-health observation. A tile is not evidence that a real browser, terminal or computer provider exists.

## 2. Conversation and composer

The Conversation is the default surface and the authoritative human-visible history. It supports normal text, supported attachments, selected native agent context, durable message history, streaming/reconnection, stop, retry and recovery.

### Composer behavior

1. User selects an eligible Workspace AgentBinding. Agent-specific model, effort, mode, commands, mentions and references are shown only when the selected AgentModule declares them for the exact installed version and platform. UI eligibility does not establish authentication, session readiness or a qualified provider route.
2. The draft remains local until submission. Switching the selected agent changes the binding used by a future turn; it does not transfer a private session or grant permission.
3. Send submits a typed Conversation turn with a stable request identity and the exact selected binding only while the local Operator is ready and the exact current Runtime/incarnation projection matches ONLINE/READY. A committed user message and an OPEN turn are not represented as a completed answer.
   When no Conversation is selected, the first Send uses `CreateConversationFirstTurnRequested` and one atomic admission; it does not create an empty Conversation and defer the text for a second click. The exact binding currently shown in the selector is pinned in the request, including the visible Workspace-default option. Text ResourceRefs selected in the New Chat composer are included in that exact request.
4. The UI shows progress only from correlated runtime/provider events. A stream gap triggers a persisted projection refresh.
5. Stop requests cancellation and waits for authoritative settlement. A local button click is not proof the process stopped.
6. Retry is available only after the prior turn's terminal or reconciled state permits it. Ambiguous dispatch locks duplicate submission until reconciliation.
7. Errors preserve the user's draft and display typed recovery guidance. A transport error after durable admission is an ambiguous result, not an invitation to submit a new request identity.
8. If Runtime lifecycle is loading, missing, stale, mismatched or not ONLINE/READY, disable only fresh Send and Retry and show the named **Refresh Runtime status** action. The daemon transaction is authoritative; refresh is only a UI preflight. Exact saved-send replay, exact committed Retry replay, Stop/cancel, settlement and reads follow their existing rules.

A Conversation does not create a Task unless the user explicitly promotes outcome-oriented work through a reviewed action. A Coworker is optional. Conversation history can outlive a provider process without claiming the private provider session survived.

### Empty-state contract (TARGET)

When no Conversation is selected or a new Conversation has no messages, the canvas shows a
product-owned empty state. The TARGET copy is truthful and LiteCowork-branded: a neutral
heading such as "Start a conversation" with supporting text that describes only what the
product actually offers (select an agent binding, type a message, attach supported
Workspace context) and no capability the current build cannot perform. The empty state
MUST NOT carry third-party "Antigravity" branding and MUST NOT advertise unsupported
affordances (file/context attachment, voice input, @-mention or slash actions) as if they
were live. Any capability named in the empty state must have a wired handler and a served
route behind it; otherwise it is omitted or shown as needs-setup from a real provider-health
observation. The current `conversations/ConversationsPage.tsx` empty canvas now meets this
target (truthful LiteCowork-branded copy, no Antigravity branding, no unsupported capability
claims; register A-26, empty-state branding resolved), guarded by
`tests/conversation-empty-state-truthfulness.test.mjs`. See the current implementation
boundary below.

### Current implementation boundary

**Source re-check 2026-11-23:** The desktop has cursor-paginated Recent Chats, create with an idempotency journal, snapshot/message pagination, local drafts, an OpenCode-only submit path, visible turn Retry, and a turn Stop control. New Chat first-send uses one atomic first-turn route and a bounded recovery journal; the exact selected/default binding is pinned before submission so retry cannot drift after a Workspace default change. T32 now adds a separate operational gate: fresh Send and Retry require Operator readiness plus an identity-matched lifecycle projection for the exact current Runtime/incarnation reporting ONLINE/READY. A visible Refresh Runtime status action re-reads the lifecycle projection when it is missing, stale or unavailable. The daemon independently rechecks the process-owned identity inside the SQLite admission/Retry transaction after exact replay; a racing failure returns 503 RUNTIME_UNAVAILABLE without a new receipt or partial write. Exact saved-send replay, exact Retry receipt replay, Stop, settlement and reads keep their existing conditions. This is CODE/SERVED readiness enforcement only; it does not establish provider authentication, native-session readiness, a qualified provider route, or a packaged UI/provider journey. A-24's former "qualified selectable binding" label remains superseded.

**Existing Conversation Send recovery (A-113 / T30, revision 1.2 IN_REVIEW):** each exact send is journaled in the same private bounded Tauri file before dispatch. On ambiguous response or restart, the composer shows a saved-send recovery notice and Retry same send reuses the same body and key. Pending text is never represented as a committed transcript row; only the authenticated Conversation snapshot can show the committed message. Draft/attachments stay pinned while outcome is uncertain. The bridge returns structured `ADMITTED` or closed typed `REFUSED` outcomes without deleting the journal row; the UI validates the outcome and explicitly acknowledges only that exact Workspace/Conversation/request identity. After admitted acknowledgement it reloads the canonical snapshot and clears the matching draft/chips. After typed refusal acknowledgement it keeps or restores the saved draft/chips for correction. If acknowledgement and the follow-up journal read both fail, the exact request remains visible in memory and new sends stay paused for safe same-key retry. The recovery notice remains visible while the journal is unavailable; a retry still uses the original identity. An unresolved row blocks a different send in that Conversation. Exact receipt replay precedes mutable Resource checks, but new admission continues to revalidate current Resource and AgentBinding access. This is a user-delegated design/implementation direction, not package ratification or provider qualification.

If a retry after restart receives a typed pre-commit refusal, the UI explicitly acknowledges that exact row only after receiving the structured refusal, then restores its text and attachment chips to the originating composer. Saved labels and Resource revisions remain pinned; a newer picker revision is not substituted. Missing Resources remain removable chips until the user explicitly removes or reattaches them. The composer then permits a corrected send under a new request identity.

Storage commits USER message/OPEN turn/dispatch intent/receipt under a strict one-active-turn-per-Conversation guard; the daemon claims the intent and the startup sweep fails unsettled prior-incarnation work closed rather than resuming opaque native state. These are CODE mechanisms, not a live OpenCode journey. The composer remains fenced for every nonterminal projected turn, transient stream, and submit in flight. Stop is offered only for RUNNING after a source audit found that the UI previously offered it for WAITING_USER/WAITING_DEPENDENCY even though the daemon rejects those states; the UI guard now hides those unsupported controls. This is a truthful fail-closed limitation, but it does not satisfy accepted AC-024; the mismatch is open in register A-79. RUNNING Stop commits CANCEL_REQUESTED, asks the provider adapter to stop, and becomes CANCELLED only after observed quiescence. The target still needs provider-aware cancellation for waiting requests/dependencies. The server route can settle OPEN as FAILED/USER_CANCELLED, but the UI does not currently offer that route as Stop. Retry has an actual UI call site and reopens FAILED through a new durable retry intent; races and packaged provider behavior remain unqualified. Turn activity remains a collapsed, explicitly agent-reported observation. The latest `activity_turn` snapshot identity lets the page reload the last turn's bounded REPORTED activity after completion/restart while keeping Stop/Retry projections separate; a failed activity read does not hide or alter the semantic answer. This is CODE/SERVED design and source behavior only, not proof that a provider truly performed an action.

The composer can attach only already-ingested, bounded text Resources as revision-pinned ResourceRefs; dispatch reauthorizes/materializes those refs. Binary files, WorkspaceRoot ingestion/retrieval, durable context receipts, and Context Used remain unavailable. Add context is therefore limited to the served Resource picker. Voice Input and More options have no active behavior; Turbo is static and @/slash is placeholder. The empty state is truthful and LiteCowork-branded (A-26). No exact native provider route, packaged desktop path, or end-to-end Conversation journey is QUALIFIED. See Feature 001 SPEC/TASKS and IMPLEMENTATION-STATUS for remaining source details.

**Explicit Task promotion (Feature 005-T12 CODE/SERVED; A-114/A-115 user-delegated direction, cross-owner review pending):** eligible committed AGENT replies offer **Continue as Task** only when the same-snapshot message wrapper reports `turn_status=COMPLETED` and a committed preceding USER message with the same `turn_id` is loaded. Unknown/missing, streaming, failed or cancelled status never enables it. The draft shows the source Conversation and Workspace, begins with the preceding USER message text, lets the owner choose an enabled same-Workspace lead-eligible binding, and lists exact pinned Resource inputs from that USER message. Unresolved refs are identified and excluded; a newer Resource revision is never substituted silently. Save creates only a READY Task and TaskSpec revision and opens that Task in Work; Work provenance links back to the source Conversation. The storage transaction rechecks source IDs, Workspace/Conversation/turn, exactly one matching canonical append event for each message, completed settlement, producer identity, lead eligibility and exact source Resource pins. The source messages already exist; promotion does not append or mutate a Conversation message or turn. Ordinary Send remains unchanged and does not create a Task. The UI holds an ambiguous request key/body in memory only; restart recovery and the packaged desktop journey remain unqualified.

## 3. Work and Task detail

Work is the durable execution workspace. A Task is distinct from a Conversation and is created only through explicit promotion or an authorized Automation occurrence.

The target Task screen contains:

- **Overview:** objective, Workspace, origin Conversation/Coworker revision, lifecycle state, active runtime, placement and blockers.
- **Plan:** immutable accepted PlanRevision, ordered Steps, dependency edges, acceptance criteria and supersession history.
- **Activity:** durable event timeline with actor, Attempt, timestamps, state transitions and correlation.
- **Workers:** lead and delegated workers, exact AgentModule/version, eligibility, scope, lease, budget, outcome and verification.
- **Files and Artifacts:** input pins, actual revision changes, committed outputs and provenance.
- **Delegation policy provenance:** an expandable, read-only Task-detail disclosure shows the exact server-resolved source, Workspace settings revision, optional Coworker revision, strategy, native child-model policy, selected worker-profile revisions and their model-selection policies. A legacy null snapshot is explicitly unavailable; this disclosure is not evidence that a worker ran or is qualified. Task specification history shows the snapshot belonging to each revision.
- **Inspector:** technical traces, tools, process/environment identity, policy decisions, receipts, tests and redacted logs.
- **Controls:** pause, resume, steer, cancel, retry or recover only when the owning service admits that exact transition.

### Mission Control and visual execution

The multi-agent execution view belongs inside Work/Task detail, principally Plan, Activity and Workers. It is a visual projection of durable Steps, dependencies, Attempts, AgentSessions, Effects, Artifacts and Evidence. It is not a free-standing animated graph or a second workflow engine.

A node may display running, blocked, failed, completed or independently verified only when the owner record supports that state. Every node opens its actual inputs, outputs and evidence. Do not invent percent complete, ETA, elapsed-time predictions, agent presence, worker count or progress animations. An ordinary user sees outcome, steps, responsible worker, result and blocker; a technical user can expand the Inspector.

### Current implementation boundary

The current Work screen lists persisted Tasks, opens Task details, supports objective/specification revision and requests planning readiness. Current and historical Task specification projections carry the delegation policy snapshot; the Work detail shows it in an expandable provenance disclosure and labels legacy null as unavailable. The readiness result explicitly states that planning and dispatch are unavailable. The current UI does not have an executing Plan/Attempt timeline or operational Task controls. A READY Task record is not a running Task. Tauri projection tests remain pending; UI markup tests cover the snapshot and legacy-unavailable states.

## 4. Needs You

On a Conversation stream reconnect, the inline native-permission card becomes non-actionable until an exact-attempt snapshot refresh succeeds. A failed check keeps the card disabled and exposes **Check live status**; this has controller/reducer CODE coverage but no rendered reconnect journey.

Needs You is an actionable inbox, not a decorative count or general activity feed. It groups persisted approvals, UserRequests, authentication actions, conflicts, human takeovers and recovery decisions. A native Conversation permission may appear only as a live Runtime-backed item; it is not restored from historical REPORTED activity and disappears or becomes ended when its native session ends. OpenCode v2.0.32 rejection is disclosed as rejecting all requests currently pending in that same session.

**Current permission implementation evidence (001-T27, partial):** the daemon now serves an authenticated live snapshot and reply route. Reply checks the persisted RUNNING turn and ACTIVE AgentSession, queues an opaque-token decision to the active Runtime owner, and reconciles provider errors without replaying the decision. The registry is bounded to 32 prompts per attempt and 256 across retained Runtime scopes; ending an attempt clears its exact-scope tokens so later turns can use the capacity. If reconciliation confirms a request is still pending, the attempted decision is cleared. An ambiguous attempted decision remains unconfirmed context if the exact attempt later closes; session closure is not provider acknowledgment. Resource labels show validated existing Workspace-relative paths or generic fallbacks, never absolute paths. This is CODE/SERVED partial only: the route suite includes negative-path tests, a synthetic authenticated POST-to-worker success/replay test, and a synthetic mailbox-close-after-dispatch race test. Both positive-path tests use seeded prompts and a fake provider: the first proves same-key receipt replay does not dispatch twice, and the second returns GONE after the attempt closes rather than reporting ANSWERED. Neither qualifies a live provider reply. The desktop uses typed Tauri commands, shows direct controls inline for the RUNNING Conversation attempt and in Needs You from a bounded Workspace snapshot, and consumes sequenced reply/session-end frames. Needs You refreshes on entry, foreground/focus return and explicit refresh; there is no Workspace-wide push subscription or durable inbox row. The snapshot is Runtime-local and available only while its persisted turn/session remains active.

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

OperatorServer starts the local AutomationTriggerHostWorker, which polls persisted cursors every 30 seconds and settles supported recurring schedules. Manual Run creates an occurrence plus READY Task; Create test Task creates READY Task without occurrence/cursor. Neither starts Task planning or agent execution. Definition list pagination has Load more, but Automation occurrence history consumes only its first API page and drops next_cursor. Feature 010 T02 corrected the stale unavailable claim. Save and pause confirmations say recurring schedule hosting is off or scheduled Task admission is stopped; resume copy says overdue slots follow the configured misfire policy at the next host poll. Manual Run eligibility depends on exactly one Manual trigger, a non-disabled Automation, an active pinned Routine, inputs that pass validation at submission, and no busy or unresolved request. `retry_policy` and `wake_policy` are saved but have no production consumer. See Feature 010 SPEC/RESEARCH for the source crosswalk.

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

### Saved Views (TARGET, owner direction 2026-11-17)

Saved Views live as a filter/group in the existing Library. They do not add a sidebar tab or separate store. The owner selected them for V1 and directed a Feature 007 B-21 amendment; Feature 007 revision 1.1 is IN_REVIEW and Feature 016 T01/T02 remain open. The target object is a bounded, non-executable Artifact snapshot with exact source Conversation/message/presentation lineage and semantic fallback. It reopens the exact committed version and original declared defaults. Session-modified calculator values remain transient under B-40; V1 has no Save changes for those values and no automatic agent sharing. Update is a separate confirmed append from a new committed eligible presentation; removal uses Archive. No control is available until the matching Feature 007 command, receipt, and projection are served.

The Library includes Workspace resources and committed Artifacts. Each item records source, exact revision, owner, Workspace, origin Conversation/Task, MIME/format, content hash, timestamps and provenance.

Each format declares its actual capabilities: read-only preview, edit, save, export, compare, render or unsupported. A preview is not an editor; a text extraction is not faithful layout rendering; PDF display is not Word editing; and a generated file is not an accepted deliverable until its required checks pass.

Editing uses a draft revision and an explicit save/commit operation with conflict handling. Restore and delete honor owner/scope fences and retention policies. Opening an Artifact after restart resolves the committed version, not a stale WebView buffer.

### Current Library / Resource surface

The mounted Library provides bounded multi-file upload/quick-import (up to 100 files / 100 MiB per selection), saved Resource list and pagination, metadata search, indexed local-text search, bounded on-demand text scan, exact-revision preview, source-reference preview, Save original, text-index rebuild, Resource revision history/edit, and Workspace Note creation. It can pin selected exact Resource revisions as Task inputs, but the UI says these are not read by an agent until Task execution exists. Search hits are not silently attached to Conversation. ZIP files are saved intact; the mounted ZipIntakeNotice reports extraction unavailable and contents remain opaque, unindexed and unattached. PDF/Office/image originals may be stored, but the current local text index/preview does not extract them. A source ZIP metadata-preview route is not called by the UI and its isolated worker is development-feature-only, absent from release sidecars.

WorkspaceRoot controls are colocated in the Library under Persistent folders. **Current source:** Add uses the native picker and immediately commits on supported Linux/macOS without a separate confirm; Windows Add remains enabled but the daemon returns 503. **Accepted Feature 003 target:** show the selected folder and policies in a post-picker confirmation step, and disable Add/Scan before selection when the current platform is unavailable. Scan is a Linux-only metadata census; Windows/macOS return 501, after which the UI disables further attempts. Observed-file browsing shows names/size/freshness and does not imply content revisions. Root scan does not ingest, watch or index files. Grounded retrieval is a separate read-only lexical endpoint; no ContextAttachment, Context Used panel or Conversation retrieval wiring exists. See Feature 003 SPEC/RESEARCH and IMPLEMENTATION-STATUS for the exact source crosswalk.

### Current Artifact boundary

The Library's Saved, Task outputs (TRANSIENT) and Archived views list already committed Artifact projections; these are Artifact lifecycle filters, not a Saved Apps collection. There is no create/generate/import-convert action or served Artifact-create route. ArtifactWorkbench reads exact immutable versions and provenance, previews selected text formats, compares exact stored versions, and permits bounded managed `text/plain` append/edit only. Markdown/CSV/JSON/XML/YAML preview does not mean those formats can be edited or round-tripped. Office/PDF uploads remain Resources, not Artifacts. Task outputs are read-only and have no live producer while Feature 005 Task execution is unavailable. Native Save As writes one exact managed blob version locally (up to 10 MiB) and creates no Artifact/Resource revision, domain event or ExportReceipt. If a committed rich presentation exists, it is revisited through its source Conversation; the current code has no production publisher. Optional device-only answer inputs do not create a Library item. Runtime app data, if separately qualified, belongs to the isolated Artifact Runtime's per-Artifact/version store and is not the Artifact package or Feature 016 answer state. Any reusable generated app must be a committed ArtifactVersion with owner-backed provenance and a durable create receipt; that path is not currently served, and its runtime remains separately unqualified. See Feature 007 SPEC/RESEARCH, Feature 016 A-103 and `docs/IMPLEMENTATION-STATUS.md`.

## 9. Workbench

Workbench is an internal owner term for the contextual object workspace, not a separate user-facing destination or specialist mode. The pane is titled by its active object (or shows a compact start surface when empty). It holds actual authorized objects, not a permanent palette of specialist tool tabs or a set of mock demos; object types and actions are bounded by their owning contracts and current provider readiness.

The target shell has an object tab strip and **+ / New tab** chooser. Tabs identify the actual object or view (for example, a file revision, diff, agent activity projection, browser session, Computer session, terminal session, Canvas/Artifact or a rendered answer), not an implied always-on tool. The chooser can search available object types or Workspace objects and explains unavailable prerequisites. A **URL** entry is offered only through a qualified Browser owner path with that provider's URL/navigation policy; it is not a generic way to open an arbitrary local path or an unreviewed agent-supplied destination. Selection, opening a reference, restoring layout and changing tabs are view operations only: they never grant access, start a process, begin capture, dispatch work, or revive authority. Reopening a persisted reference reauthorizes it against the current Workspace and owner state. A bounded overflow/close menu and keyboard navigation keep the strip usable with many tabs; the left navigation remains compact.

**User-directed navigation implementation (A-123, 011-T19; not formal package acceptance):** the target shell has **New Chat**, the existing **Recent Chats** list, **Projects**, **Needs You**, **More**, and footer **Settings**. Projects is initially a directory over existing Workspaces, not a new Project record or security boundary; selecting a Workspace changes the active Workspace context and does not itself grant authority. Work, Coworkers, Library, Automations, Discover, Dashboard, Goals, Notifications, Ideas, Routines and explicit New Task remain reachable through More. Do not show a composite Needs You count until persisted inbox items and Runtime-local native prompts have a defined deduplicated snapshot. Do not label the existing Conversation-only sidebar list as unified Recents: a unified Conversation/Task/Artifact feed still needs ordering, pagination, partial-source freshness and error contracts. The accepted four-destination baseline remains the formally accepted release contract until owner review. See [Workspace Experience Revision](WORKSPACE-EXPERIENCE-REVISION-2026-11.md) and A-123.

At narrow widths the Conversation header must provide an explicit Menu button because the standard page topbar is absent on that route. It opens the shared navigation drawer, marks `aria-expanded`, moves focus into the drawer, and restores focus to the matching trigger after Escape or an accepted route selection. If the Library dirty-draft guard blocks a route request, the drawer closes without moving focus into inert content; Stay restores focus to the active screen's Menu trigger, while explicit Save/Discard continues the queued route with the drawer closed. The 2026-11-24 `shell-reachability` rendered browser CODE run exercised focus entry/return and dirty-guard recovery in Chrome 158.0.8127.38. This does not constitute packaged Tauri or platform qualification.

This is the product's broad workspace direction, not a claim that LiteCowork replaces desktop applications or controls unmediated native-agent tools. An optional **Open with** handoff for a file is a separate future owner decision: it must pin the exact file revision, use an explicit user gesture, identify the external application, and state that external edits are not LiteCowork-mediated or automatically reconciled. It is not part of the V1 promise in this contract revision.

| View | Required behavior | Current boundary |
|---|---|---|
| Start | Surface available providers and entry points with truthful health | Launcher tiles do not imply service availability |
| Files | Workspace resource tree/details and exact-revision preview; edits use a qualified mutation path | Current list/preview is not a general filesystem editor |
| Browser | Real managed-browser session, isolated profile, navigation, authorized observations, lease takeover and return | Current pane is an empty-by-default embedded-page preview, not a Chrome/CDP browser session. Browser observation remains its own tab viewport and lease; it does not provide the requested laptop-wide Computer view. |
| Computer | Real selected display/window, live local view, independent agent-disclosure consent and optional mediated input with immediate takeover | Current source remains UNAVAILABLE with status only. Prior B-32 deferral is recorded; owner-requested Feature 008 revision 1.1 proposes the actual laptop view/takeover but is IN_REVIEW and has no provider, route, live UI or qualification. |
| Terminal | Real daemon-owned PTY/ConPTY, process identity, stream, resize, bounded scrollback, detach/reattach, stop and recovery | Current pane is explicitly unavailable and runs no shell or command. |
| Diffs | Actual VCS or revision-to-revision diff with file identity, hunks and provenance | Current pane states that no change set is served; it shows no synthetic diff. |
| Activity | Persisted events, provider reports and verified records with assurance level | Current pane states that no Activity source is served; it shows no synthetic token/process traces. |
| Artifact preview/edit | Format-specific renderer/editor and revision commit | Only enable features proven by that Artifact handler |
| Inspector | Redacted execution, authorization and recovery detail | No secrets, native handles or unverified state |

The table lists object families, not a required fixed tab row. Show a tab only for an open object/view, and only show actions supported by its owner. Agent activity can expose correlated, provider-reported public status where a supported projection exists; native private subagent transcripts, hidden reasoning and unsupported controls are not surfaced. A live Computer view is the requested target when independently permitted and qualified; status-only UNAVAILABLE remains the current behavior. The app-chrome Take over/Stop run strip remains independent of selected tab while a mediated Computer input lease is active.

A terminal session belongs to the daemon Environment, not the React component. Switching tabs detaches the view; it does not kill the process. Runtime restart marks an old PTY lost and never silently reruns the command. Bounded output retention, backpressure and explicit termination are mandatory.

A Browser iframe preview must not be labeled a sandboxed agent browser or used as evidence of browser control. A true browser controller has its own OS process/profile, egress policy, session identity and takeover contract. The Browser target viewport remains the bounded managed-browser tab. A separate user-requested Computer view would capture a system-selected display/window; that change is proposed in Feature 008 revision 1.1 and remains IN_REVIEW pending owner resolution of the prior B-32 deferral. Browser Take over transfers only browser-input authority; Computer Take over requires a separate, fully mediated desktop-input provider. Conversation Take control separately stops the active turn through Feature 001 quiescence before owner continuation (Feature 012 B-34).

### Current Workbench source boundary

**Source re-check 2026-11-12:** The Files view is a Workspace Resource projection/preview, not a live filesystem browser. Browser is an empty-by-default embedded-page iframe, not Chrome/CDP and not a managed browser session. Terminal and Review are status-only unavailable/empty views; there is no shell process, fabricated exit status, or synthetic diff. Activity has its own status-only empty view and does not fall through to Review. Computer use is status-only UNAVAILABLE with no capture/input provider. No BrowserSession, terminal, Activity, Diff or Computer provider route is served. Browser-tab observation remains target design only. The separate request for a selected laptop display/window view and immediate mediated input takeover is recorded in Feature 008 revision 1.1 and `docs/COMPUTER-OBSERVATION-AND-CONTROL-LLD.md`, both IN_REVIEW; it does not override B-32 or count as implementation evidence. See `docs/IMPLEMENTATION-STATUS.md` for exact CODE/SERVED/evidence levels.

## 10. Intelligent UI and presentation

Every answer keeps a durable semantic message. Presentation enhances that answer; it does not replace semantic truth.

The target presentation layers are:

1. Semantic response: ordinary text/Markdown and committed conversation message.
2. Rich presentation: typed cards, callouts, tables and structured summaries.
3. Generated media: MIME-validated image/media Artifacts with hash and provenance.
4. Intelligent UI: schema-validated declarative tables, charts, calculators and structured interfaces using registered components and restricted state reducers.
5. Interactive Artifact: a self-contained application executed only inside the separately qualified isolated Artifact Runtime.

The preferred agent interface is a typed structured output/tool event delivered through a provider-qualified path. ACP is transport, not a universal UI schema: its content blocks and tool-call events do not define a component tree, and an optional MCP server supplied at session setup does not guarantee that an agent will use it or emit a dashboard. An ACP route may propose UI only after that exact agent/version/auth path is qualified and its payload maps to Feature 016's canonical RichPresentation/PresentationIntent contracts; the host validates before display. A text fence may be accepted only as untrusted input that passes the same schema and size/depth validation. Reject unknown components, arbitrary script, network imports, host APIs, event handlers or hidden authority. Bind the presentation to the committed semantic message and content digest. The renderer may use only registered component types and bounded local state. Provide accessible text/table fallback. ACP content and session setup are defined by the [v1 content](https://agentclientprotocol.com/protocol/v1/content), [tool-call](https://agentclientprotocol.com/protocol/v1/tool-calls) and [session-setup](https://agentclientprotocol.com/protocol/v1/session-setup) contracts; LiteCowork's provider-specific bridge remains separately gated by Feature 016.

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

### Current source boundary (rechecked 2026-11-24)

The mounted `App.tsx` Settings route exposes four categories: General, Workspaces, Agents and Runtime. The 16 categories in §14 are target IA, not current navigation. Appearance preferences are persisted as local UI preferences and applied through the root theme attribute; global dark tokens exist, but some Settings and Workbench component styles still use fixed light colors, so dark-theme visual completion is partial. The shortcut registry displays New Chat and Library/search; its handlers accept Ctrl or Meta, while Ctrl+W is intentionally not registered because the WebView/OS reserves it. Workspaces exposes selection/refresh/create, instruction revision editing, archive, rename, policy changes and local Runtime enrollment; WorkspaceRoot controls are in Library. Enrollment records the local Runtime role for a Workspace and does not start an agent or execute a Task. Agents exposes bounded catalogue/profile-probe/binding operations; native sign-in/config openers remain fail-closed. Credential submission now has a CODE/SERVED authenticated daemon route with durable replay and idempotent SecretStore behavior, but current V1 descriptors declare no LiteCowork-owned slot, so no sign-in UI or provider path is available. Runtime exposes start/refresh and lifecycle observation; the visible card does not offer stop/drain or startup-policy mutation. The exported `SettingsPage` function in App.tsx is unmounted. See [SETTINGS-AND-SHELL-SURFACE-AUDIT.md](SETTINGS-AND-SHELL-SURFACE-AUDIT.md) for the current control crosswalk. The Runtime UI's “Connected (Localhost)” label is inconsistent with the actual OS-local socket/named-pipe transport and must not be repeated as a transport fact.

## 12. Global interaction and accessibility rules

- Every actionable control has a real command, defined disabled state, loading state, success receipt, error and recovery.
- No button is enabled solely because a component or enum exists.
- No optimistic state is authoritative for approval, installation, execution, effects or task completion.
- Preserve drafts on failure. Bind retries to the original idempotency identity when the owner says the outcome may be ambiguous.
- Use source and freshness labels for observed capabilities; show unknown distinctly from empty.
- Respect keyboard navigation, focus visibility, screen readers, reduced motion, contrast and non-color status.
- Motion illustrates real state transitions and must stop for reduced-motion preference. Do not animate invented worker activity or fake progress.
- Keep the default surface plain-language and progressively disclose model/version, policy, process, lease and trace detail.


## 14. Settings, Agent profile and Usage — V1 release surface

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

## Requested computer and intelligent-answer revision - 2026-11-12

The [experience revision](WORKSPACE-EXPERIENCE-REVISION-2026-11.md) owns cross-screen navigation and traceability. The right panel uses open object tabs plus a + menu for Computer, Browser, Terminal, Files, Changes and Canvas. When a provider-backed Computer session is already LIVE with local preview permitted, its Computer tab becomes the visible Workbench selection so the user can watch the current screen; the view change never starts capture or grants agent access. A deliberate close/collapse is respected for that session without hiding the persistent control strip or **Show computer** entry. Provider-backed observation includes the selected real screen/window; Browser remains a separately scoped managed tab. The in-app Take over strip is supplemented by a qualified out-of-app global shortcut or native tray action because LiteCowork is behind the target app during computer use; without one, mediated Computer input is unavailable. Both triggers close the same broker fence before showing transfer completion; pending/unknown dispatched actions stay visible. Sending a message does not transfer control. Return to agent is explicit and requires fresh observation. Detailed provider and terminal behavior stays in Feature 008.

Inline and expanded Canvas retain document identity and current permitted local state. No resize/tab interaction creates a new Task or sends values to a model. Saving persistent view state or conversationally changing its definition is separately versioned under Feature 016. Arbitrary executable apps remain preview-only until their qualified runtime exists. The stale Current implementation boundary paragraphs in older sections must be read against IMPLEMENTATION-STATUS and current source; they are not qualification claims.
