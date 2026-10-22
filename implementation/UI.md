# Desktop UI delivery, control inventory and interaction acceptance

**Owners:** [EXPERIENCE](../docs/EXPERIENCE.md), [COWORKERS-TARGET](../docs/COWORKERS-TARGET.md), [DESIGN-SYSTEM](../docs/DESIGN-SYSTEM.md), [MOTION](../docs/MOTION.md) and [PRESENTATION-RUNTIME](../docs/PRESENTATION-RUNTIME.md).

**Status:** Accepted target design, NOT a claim that the current Tauri/React UI or its supporting Operator routes implement every row. Current source includes a Workspace Home/Task composer, read-only Conversation catalog and Coworker settings editor. Provider-backed Conversation send and Coworker-owned conversations are not yet available. Never make a disabled control look live or replace it with a simulated response. The direct ordinary chat must be accessible without creating a Coworker; a Coworker is an optional persistent chat-first specialist.

## IA and first-run contract

~~~text
Main sidebar
  + New session                   ordinary Conversation, selected native chief
  Search / recent conversations   Workspace-scoped
  Coworkers  [expand]  [+ Create] optional roster
    Research Assistant            opens last active same-Workspace chat
    Travel Planner                opens last active same-Workspace chat
  Needs You                        outstanding actual approvals/requests
  Work                             all canonical Tasks and verified outputs
  Automations                      Routines, Automations, occurrences, runs
  Library                          Resources and Artifact library
  Discover                         apps, MCP servers, skills, qualified capabilities
  Settings                         Agents (chief), Subagents (external profiles), Runtime
Coworker contextual destinations
  Chat | Responsibilities | Work | Memory & Knowledge | Connections & Tools | Settings
Workbench: resizable on right, initially closed; never discard unsaved edits.
~~~

Main composer: generous multiline input, accessible Add, native chief/model/effort, Send/Task/Schedule options and Workbench open state; availability is derived from actual adapter capability. A one-off user may never interact with Coworkers. Task creation and Chat sending are distinct actions even if they look like one natural-language experience. Do not silently create a Task, schedule or saved memory during a simple conversation unless the appropriate accepted policies exist. The current Home save-Task flow may remain until turn admission exists, but it must never claim a live native chat.

First run: choose/create Workspace, select and authenticate a compatible native lead from supported provider options, then start ordinary conversation. The user MAY create a Coworker or add connections; neither is mandatory. A lack of eligible native lead preserves draft and shows one concise repair. No global provider router; agent settings are native to the selected harness.

## Full surface and owner map

| Surface | Default | Controls | Critical failure states | Owners |
| --- | --- | --- | --- | --- |
| New session/chat | Blank ready conversation; no compulsory assistant roster | Chief/model/effort; Add; Send; Run as Task; Schedule; Save Routine; Stop/retry if supported | No selected/eligible/authenticated lead, offline, session blocked, stale turn, unsupported native option | E03-S01/02, E08-S04 |
| Coworker roster | Compact optional expandable sidebar | Select Coworker, + New Coworker, contextual New chat/Settings/Archive | Empty/archived/partial list, stale last activity, changed Workspace, deleted owner | E08-S01 |
| Coworker chat | Last active chat or empty centered composer | + New chat, conversations history, native chief selectors, activity cards, Workbench | No associated Conversation API, unsent draft, paused Coworker, missing chief | E08-S01/04 |
| Quick Create | Two fields (name/purpose), avatar optional | Create, Customize, Cancel | Duplicate request/response timeout, invalid name, Workspace change, no lead | E08-S01 |
| Customize Setup | Optional progressive path | Identity; chief/model/effort; external workers; Memory & Knowledge; Connections; Responsibilities; Review | OAuth interrupted, provider unsupported, form conflict, partial draft, skipped section | E08-S01/03, E10 |
| Connections & Tools | Searchable, paginated catalog, no arbitrary configured count ceiling | Search; tabs; Connect; Assign existing; Inspect; Manage; Disconnect; Repair | Offline/local app unavailable, auth expired, reused account, digest changed, unqualified native bridge | E04/E10 |
| Responsibilities | Secondary compact list | Review, Turn on, Edit, Pause, Resume, Run now, History, Archive | No trigger provider, DST gap, asleep, duplicate event, revoked source, overlap, budget | E09-S01/02 |
| Work and run details | Canonical Task and verified output | Inspect, Pause/Resume, Cancel, answer Needs You, open Artifact | provider ambiguous, missing artifact, stale lease, unsafe retry, offline OS | E03/04/07 |
| Memory & Knowledge | Quiet scoped source/record list | Inspect source, revision, correct, revoke, delete, filter, view Context used | no extractor, temporary chat, conflict, pending purge, stale provider data | E08-S03 |
| Global Needs You | Pending actual requests only | Open exact request, Approve/Deny/Answer/Sign in/Take over | resolved/expired/stale approval, wrong principal assurance, offline response | E02-S04, E04 |
| Workbench and outputs | Right context panel, closed by default | Preview, compare, edit, Save As, restore version, open source | dirty navigation, revoked artifact, unsupported format, digest mismatch | E08-S07 |
| Runtime/background | Shows real status and availability | Start, launch at login, keep background, stop/preview drain | sleep/lock/offline/crash, existing AMBIGUOUS actions, poweroff | E01/E07/E09 |
| Global Automations | Cross-Coworker canonical list | Filter, new, edit, pause, run now, create test Task, view trigger/occurrence | invalid scope, missed slots, duplicate run/test, host offline, disabled/archived | E09 |

## Interaction inventory: exact action, prerequisites, outcomes, recovery

### Sidebar and navigation

| Control | Eligible state and effect | Ineligible, retry and keyboard behavior |
| --- | --- | --- |
| + New session | Creates or opens an ordinary Workspace Conversation; selects native lead without Coworker assignment | Requires authorized Workspace; preserve draft on switching if nonempty; blank Workspace shows setup |
| Search | Search permitted conversation/resource metadata within current Workspace | Empty/no access/partial results distinguished; never index hidden Coworker notes in global scope |
| Select Coworker | Open last active Coworker-owned Conversation, ordered by authoritative server activity | On missing owner route show unavailable rather than unrelated chat; cancel stale navigation requests |
| + Create Coworker | Open Quick Create with focus on name, role hint and optional Customize | Disabled only if no writable Workspace; Escape/cancel restores previous focus |
| Coworker overflow menu | New chat, Conversations, Responsibilities, Work, Memory & Knowledge, Connections, Settings, Pause/Archive | Archived row is read-only; do not offer unavailable actions as fake buttons |
| Global Needs You | Opens actual pending approval and UserRequest projection | Stale count rechecks backend; no invented zero for unknown |
| Global Work/Automations | Open canonical Task/Automation record, not a duplicate under Coworker | Cross-Workspace deep links rejected; last route restored only if authorized |
| Workspace selector | Switch scoped workspace and reload authorized views | Prevent in-flight stale result from rendering; preserve or confirm unsaved drafts |

### Chat and composer

| Control | Action and result | Edge behavior |
| --- | --- | --- |
| Native chief selector | Choose eligible enabled harness for FUTURE turns | No global model provider; unavailable choice disabled; current Attempt remains pinned |
| Model and effort | Show actual harness-supported model/effort options | Stale model entitlement is UNKNOWN; option change cannot claim agent successfully switched until provider confirmation |
| + Add | Files, folder, screenshot, existing Library resource, qualified Connect app | Permitted Resource identity pinned; rejected size/type/scope; no implicit permission to ingest everything |
| Send | Admit ConversationTurn and message when provider-backed turn service qualified | Preserve draft on network failure or unsupported Session controls; no fake AI answer |
| Run as Task | Create durable Task with typed TaskSpec, acceptance/budget from input | Receipt is distinct from execution; pending/idempotent retry must not duplicate |
| Schedule | Show reviewable Routine/Automation/trigger/timezone/location/approval before enable | Invalid trigger, missing host/connection, cancelled review leaves no enabled work |
| Save as Routine | Editable draft, sanitized context, explicit Save | No automatic Skill creation or execution; cancelled draft inert |
| Stop | Request actual provider turn/Task cancellation using scoped control | “Stopping” until acknowledged; unconfirmed external Effect remains ambiguous |
| Retry | Retry same logical request only with same idempotency identity where supported | Never silently repeat a consequential send; stale native sessions start fresh |
| New chat in Coworker | New Conversation with same owner, distinct AgentSession and transcript | Eligible memory reused by policy, not earlier transcript; temporary chat excludes memory learning |
| View activity | Show actual committed Task/Attempt/Invocation data | Empty if no work; reported versus observed versus verified clear |
| Open output | Open exact ArtifactVersion on right | Dirty Workbench prompts Save/Discard/Keep open; unsupported type has download/raw fallback |

### Create and edit Coworker

| Control | Action | Important edge |
| --- | --- | --- |
| Name/purpose | Quick Create fields, name <= 120 chars and role <= 2000 chars under existing schema | Trim and validate; duplicate name permitted unless actual service disallows; never promise uniqueness without contract |
| Create | Idempotent Coworker create; then navigate to its eligible chat after owner API exists | Ambiguous response retries same RequestId, no duplicate; if no lead, create identity but show repair |
| Customize | Opens optional settings (native chief, eligible workers, memory, integrations, responsibilities) | No mandatory connector, memory permission grid or repeated per-tool toggles |
| Skip | Skip optional section without creating an underlying connection or trigger | Save draft if UI supports draft storage; otherwise honestly say leaving loses unsaved form |
| Edit/Save revision | Expected-version update of future defaults | Stale revision 409 keeps form; review changes before overwrite; existing Task pins intact |
| Pause new work | Fence new unattended admission | Existing Task continues; wording never implies stopped execution |
| Pause all safely | Requests pause on all linked Tasks plus admission fence when qualified | Remains pending for running Effect; no immediate kill or false completion |
| Resume | Revalidates subscriptions, source credentials, Runtime and safety policy | Unsupported or revoked dependency blocks resume |
| Archive | Confirm impact; archive when backend authorizes and dependencies settled | Never deletes unrelated Tasks, Artifacts, shared installation or memory automatically |
| Delete (future) | Separate explicit categories: chats, memory, artifacts, credentials | Provider purge receipts and dependency previews; do not imply immediate global erasure |
| Switch default lead | New binding for future turns/tasks | No native-session teleportation; exact old Task provenance preserved |

### Add Connection flow: apps, MCP, skills, local apps, knowledge

Normal consumer path: Find -> Connect/install/sign in if needed -> Done. The source may be added during Customize or later through Settings or a Task-specific blocker. Configuration supports any number of eligible connected entries; only relevant capabilities become active during one turn.

| Control/state | Required UI and behavior | Edge cases |
| --- | --- | --- |
| Catalog search and category filters | Apps, MCP servers, Skills, Computer/local apps, Knowledge | Paginated/virtualized; empty, loading, failed discovery and no compatible option explicit |
| Connect existing | Assign existing authorized account/server to Coworker | Do not reinstall/re-auth unless provider requires; cross-Workspace unauthorized reuse denied |
| Sign in | System/provider-owned secure auth with trusted origin and exact app name | Cancel, timeout, invalid redirect, revoked token; never request secrets in chat |
| Add MCP | Existing server or qualified install/custom config path | Trust/publisher, remote endpoint origin, local process constraints, transport version; no auto-activation of every tool |
| Add skill | Inspect exact manifest, digest, owner and declared requirements | Skill text never grants other tools; changed digest requires review |
| Add local app | Show qualified provider and actual interaction method | Executable presence alone != supported automation; locked UI/OS prompts explicit |
| Add Knowledge | Pin approved folders/docs/repos or connected source subset | Read/search differs from eligible memory learning; indexing readiness may be pending |
| Manage access | Optional exact accounts/resources/operation classes/approval modes | Never treat display settings as enforceable when provider bypasses Core |
| Remove from Coworker | Revoke only its assignment | Do not remove global installation or revoke other Coworkers |
| Disconnect globally | Separate reviewed action with dependent assistants/tasks listed | Active invocations may be ambiguous; require safe drain or explicit pending state |
| Test connection | Nonconsequential provider readiness probe | Do not send messages or mutate external state as a “test” |
| Task-time Connect | Persist blocker, authenticate, assign, revalidate exact Task and resume | Never replay prior Effect without checking idempotency; user can cancel without losing Task |

### Responsibilities, autonomy and notifications

A new Coworker begins with Ask me for unattended behavior. Automatic memory is a distinct independent policy. Read-only monitoring and approved routines require explicit user enablement.

| Control | Action | Failure/confirmation |
| --- | --- | --- |
| Propose from chat | Host renders What / When / Where / Needs You + sources + budget | Proposal inert until user turns on; content is untrusted |
| Edit proposal | Adjust trigger/timezone/provider/scope/budget before enabling | Changes revoke stale approval; unable to persist incomplete trigger |
| Turn on | Authenticated enable of standing group + trigger readiness/outbox | Unsupported host, missing connector, bad timezone, stopped Runtime => blocked |
| Keep an eye on things | Read-only monitoring under approved bounded sources/cadence | No new source or unbounded LLM heartbeat; no-change = quiet observation |
| Take care of approved routines | Execute defined actions with narrow standing authority | Exact target/scope/approval recheck; cannot bypass payments/deletes |
| Pause | Stop new trigger admissions with version-fenced event | Existing Task not implicitly cancelled |
| Pause all safely | Pause new + request Task pauses; pending until safe | No fake immediate stop on external send or active browser |
| Run now | Distinct manual occurrence with exact pinned revision | Never enables paused recurrence or repeats a previous send inadvertently |
| View history | Actual occurrences, Tasks, outputs/evidence, missed slots, blockers | Stale/partial cursors display unknown, not zero; duplicate event dedup |
| Change schedule | New Routine/Automation revision affecting future occurrences | DST gaps/folds, timezone invalid, overlap policies tested |
| Notifications | Blocker, significant change, completion, routine digest, quiet hours | Only meaningful result; dedupe across app/tray/channel; muted still auditable |

### Memory and knowledge

| Control | Action | Edge behavior |
| --- | --- | --- |
| Memory & Knowledge | List permitted Resource-backed memories and connected searchable sources separately | Automatically learned PRIVATE by default; shared scopes require explicit policy |
| View source | Show exact source revision, provenance and freshness where available | A retrieval hit doesn't mean the agent read a full document |
| Correct/edit | Versioned supersession of a memory record | Conflicting claim marked unresolved; history remains reviewable |
| Don't remember this | Remove from future extraction eligibility for that source/turn under policy | Does not erase external native-agent prompts already sent |
| Temporary chat | No learning extraction even from delayed Task summaries | Existing audit retained under Task policy |
| Revoke/delete | Fence future retrieval and begin explicit provider purge where applicable | Tombstone immediate; deletion receipts/status can remain pending |
| Quiet indicator | Subtle, only for committed significant memory changes | No recurrent modal, no synthetic cognition animation |
| Context used | Show actually transmitted attachments/retrieval receipts | Unknown provider provenance must be labeled unknown |

### Workbench and technical surfaces

- Preview, Edit, Compare, Save As, Restore as new version, Close: operate only on exact authorized immutable revisions; dirty drafts require protected navigation and conflict recovery.
- Browser/desktop Take control and Return control: depend on committed control lease epoch. Do not show Agent moving a cursor when structured API used.
- Inspector: method, Runtime, Task, Attempt, invocation, Effect, grants, evidence and source attachments; privacy-safe redacted values, no invented quota/progress.
- Global Stop unattended work: show exact active schedules, workers, external operations and rollback limits before issuing a stop. A process exit is not proof that an external effect was cancelled.

## Error and focus matrix

| Condition | Visible state | Focus/retry |
| --- | --- | --- |
| First run with no Coworker | Ordinary composer ready if agent eligible | Primary Create Coworker remains optional |
| No eligible native lead | Draft retained and setup CTA | Focus on lead repair, do not submit |
| Conversation sending is not implemented for this build | Name that sending is unavailable; label text as an unsent, unsaved window-only draft; do not offer agent setup as a repair unless a real missing binding/auth state is known | Settings never auto-sends the draft; retain it across in-app navigation |
| Runtime disconnects after Conversation history loaded | Keep the last loaded same-Workspace list and messages visible as read-only with a clear offline label; mark them stale until a successful reconnect refresh | Disable refresh, pagination, and new writes while Runtime is unavailable; never turn an unreadable list into an empty state |
| Runtime unavailable before Conversation history loads | Explain that saved history cannot be checked while the local Runtime is unavailable | Do not claim the Workspace has no saved conversations |
| Loading Coworker last chat | Skeleton only while awaited; no fake chat | Cancel stale request on roster switch |
| Coworker offline/revoked | Named blocked state and read-only saved history | Retry exact read; do not auto-open other owner's chat |
| OAuth cancelled | Connection remains unassigned | Return focus to Connect card |
| Credential expired mid-run | Durable Needs You | Sign in; revalidate Task and Effect before continuing |
| Stale expected version | Save rejected, form and diff preserved | Reload latest/merge/retry deliberate |
| Trigger missed while asleep | Missed-run policy shown, never claimed complete | Reconcile and show actual next run |
| Duplicate webhook/retry | Same canonical occurrence/Task receipt | No duplicate row or notification |
| External send ambiguous | “Needs reconciliation”, not retry automatically | Explicit provider state or human resolution |
| Workbench unsaved content | Save/Discard/Stay dialog | Focus back to editor if Stay |
| Revoked Resource open | Hide bytes/fence new reads, keep provenance | User may inspect tombstone or restore if allowed |
| Large connector inventory | Progressive search with bounded results | Virtualize; no per-item loading of model schema |
| Low-confidence result | Not finished/Needs review | Do not style as Verified |
| Quiet hours | Notification delayed or bundled | Needs You remains accessible globally |
| Backend API unavailable | Honest unavailable label and explanatory disabled control | No placeholder mutations or synthetic chat |

## Keyboard and visual quality acceptance

- Sidebar and Coworker roster support Tab/Shift+Tab; Enter/Space activates selection; active destination has aria-current; names truncated visibly but full accessible label. Context menu opens via keyboard.
- Modals restore focus, Escape closes only safe dialog, and destructive confirmation names exactly what is affected.
- Composer Enter/Shift+Enter semantics documented and user-configurable; Ctrl/Cmd+Enter is explicit send where supported. Screen readers announce settled output, not each transient token.
- Resizable right Workbench supports keyboard resize/close and displays unsaved-draft warning on Coworker or Workspace navigation.
- No color-only status, fake ETA/percentage, social avatars pretending to think, or location/status inferred from model words.
- WCAG AA contrast and focus, 200/400% zoom, reduced motion, high contrast, long multi-language names, keyboard-only task + approval + takeover.
- All pages show empty, loading, partial, retryable failure, offline, permission denied, unavailable provider, stale reply and success states.
- At width under 900px the secondary Workbench is a route/sheet; below 600px dialogs fill the screen and touch targets are at least 44px.
- Night/day theme, 50+ roster entries, 100+ connector entries, 10k tool metadata search results, huge output artifacts, and user-switch races must not freeze UI.

## Release evidence and implementation distinction

A React mock with static cards is visual exploration only. Route and API actions require committed integration contracts and real backend receipts; do not claim running native chat or automatic memory until it is wired. Test levels are (1) pure model and component, (2) API contract/negative tests, (3) real local daemon+Tauri, (4) host OS and external provider/owner acceptance. E08/E09 stories remain IN PROGRESS/PLANNED until their own complete code, system and owner USER evidence passes. Qualify Windows, Linux and macOS background/window/sleep behavior individually. No future cloud execution promises in a local-only release.

Existing E02–E10 features remain in scope: Workspace local-only setup, persistent Resource Library and ZIP intake, Task Work/Live Desk, Goals and Suggestions, Agents/Subagents, approval inbox, provenance-rich Artifacts and Workbench, Browser/Computer takeover, Discover/Skill installation, scheduling, and final production accessibility/security/packaging.


## Right Workbench shell — finalized 2026-10-10

The supplied desktop reference is adopted as the EMPTY Workbench pattern, not as an always-open default. The useful parts are the calm single-surface layout, strong central launch actions, compact title row, and persistent layout controls. LiteCowork keeps chat as the focal surface and opens the Workbench only because the user explicitly asked for a surface or restores a user-owned prior pane state.

### Open and restore policy

- Brand-new ordinary chat: Workbench closed. This preserves reading/composer width and avoids making terminal/browser look mandatory.
- Existing window/Workspace: restore the user's last explicit open/closed state and width, provided the referenced pane is still authorized; otherwise show Start.
- Tool/background result: never steal focus or auto-open. The message/activity row offers **Open in Workbench**. Exception: an explicit user command whose requested outcome is to open Browser, Computer, File, or Artifact may open it because that navigation itself was user intent.
- Coworker/session switch: preserve the panel shell, but only keep an active object when the user remains authorized and there is no dirty-draft conflict. Never show one Coworker's private object as if it belongs to another.
- App restart: restore layout preference, not stale authority. Re-resolve the object/provider before rendering content.

### Empty Start view

Header: semantic title **Start**; back/forward only when real panel history exists; right-side controls for split-ratio preset, maximize/restore, and close. Close restores focus to the control that opened the panel. The body uses one vertical launcher list with generous whitespace, not nested dashboards.

Default launch rows, shown only when actually supported:

1. **Workspace files** — browse/search authorized Resource roots and pinned Library items.
2. **Browser** — opens the qualified built-in/controlled browser. If no browser provider is ready, keep the row discoverable only when repair is useful and name the reason.
3. **Computer** — shown when a qualified desktop/computer-use Environment exists. It opens live view and Take control; it is not synonymous with Browser.
4. **Terminal** — advanced/developer capability. Hide from the ordinary Start list unless the selected native harness/Workspace exposes a qualified terminal, or place it under **More**.
5. **Recent outputs** — optional compact row when actual authorized Artifacts exist; never fabricate recents.

Connections, MCP servers, Skills, and permissions are NOT Start-view launchers. They belong to Discover/Connections & Tools or appear as a task-time repair card.

### Geometry and responsiveness

- First explicit desktop open: target roughly 42% of usable content width, clamped to a comfortable tokenized minimum and about 72% maximum.
- Pointer and keyboard resize; remember width per window/Workspace.
- Maximize makes Workbench the content surface while retaining global app chrome; Restore returns the exact prior width.
- Below the responsive breakpoint, Workbench becomes a full-screen route/sheet rather than crushing chat beside it.
- No nested horizontal scrolling in the shell.

### Surface contract

One shell hosts File/Artifact, Browser, Computer, Terminal, MCP App, and generated App views. Surface-specific toolbars appear below the common header. Do not create a second right sidebar for Inspector or Context. Multiple Workbench objects use bounded back/forward history rather than browser-like tab sprawl by default.

Every surface declares authoritative object/provider identity and freshness, read-only/edit/control mode, actual save/publish state, loading/offline/revoked/reconnect state, dirty-draft close semantics, keyboard focus target, and whether human or agent owns the control lease.

The Workbench is a presentation and interaction host, never an authority source. It cannot turn a preview into a grant, infer browser success, or mark a Task verified because an Artifact rendered.

### Screenshot-specific decision

Adopt: compact **Start** header, spacious neutral dark/light surface, 3–5 large launcher rows, shortcut hints on the right, maximize/close controls, and clear icon + title + one-line description. Do not copy: an always-visible Terminal card for every user, unexplained layout glyphs, or a permanently open panel. Every icon control needs a tooltip and accessible name.


## Competitor-derived Coworker details refinements

- Coworker details are secondary to chat and may expose compact **Responsibilities**, **Library**, **Memory & Knowledge**, **Connections & Tools**, and **Settings** destinations. Do not turn them into a dashboard landing page.
- Responsibility row context actions: **Test run**, **Run now**, **Pause/Resume**, **Edit**, **History**, and **Delete/Archive** according to real API capability. Test run explicitly warns that it performs real work.
- A task blocked by an unconnected app shows one inline **Connect <app>** repair card. On successful auth, the card settles and the original Task revalidates/resumes; no new duplicate chat/task is created.
- Per-Coworker notifications are compact settings, not a wall of switches in creation.
- Coworker Library groups exact files/pages/Artifacts by Today / This week / Older only when those refs exist; it never implies all computer files are shared among Coworkers.
- Optional Inspector/Trace is for technical users. Default activity remains sentence-first (`Read Drive`, `Ran tests`, `Drafted report`) with duration/outcome; raw invocation payload and native reasoning are not default UI.


# Agent Registry and agent-adaptive composer target

This section is normative for the final V1 UI target and supersedes older mock/settings
wording that shows a standalone **Providers** screen, a separate **Installed agents**
screen, or one combined Agent/Model/Reasoning chip.

## Routes

~~~text
/settings/agents
/settings/agents/:registryAgentId
/settings/local-models
/settings/subagents
~~~

There is no /settings/providers.

Settings > Agents is the **Agent Registry**. It renders the live/cached ACP Registry plus
local AgentModule, installation, auth/control-descriptor and Workspace-binding state.
The URL identity is an agent/registry identity, never a provider-account ID.

## Composer component tree

~~~text
Composer
  ComposerInput
    NativeCommandAutocomplete?
    NativeReferenceAutocomplete?
    AttachmentTokens[]
    IncompatibleDraftBanner?
  ComposerControlRow
    HostActionButton(+)            # LiteCowork namespace
    AgentPicker                    # root dependency
    AgentPrimaryOptionControls[]   # Model / Reasoning / Effort / Mode
    NativeSurfaceButton            # slash / @ / other reported native affordances
    SendButton
~~~

### ComposerAgentSurface target

~~~ts
type ComposerAgentSurface = {
  bindingId: string;
  descriptorDigest: string;
  agentLabel: string;

  primaryOptions: Array<{
    optionId: string;
    label: string;
    semanticHint: "MODEL" | "REASONING" | "EFFORT" | "MODE" | "OTHER" | null;
    valueSchema: JsonSchema;
    selectedValue: unknown;
    mutableScope: "LIVE_SESSION" | "NEW_SESSION" | "UNSUPPORTED";
  }>;

  input: {
    text: boolean;
    images: boolean;
    files: boolean;
    resources: boolean;
  };

  slashCommands: NativeCommandDescriptor[];
  referenceKinds: NativeReferenceKindDescriptor[];

  setupState:
    | "READY"
    | "NEEDS_INSTALL"
    | "NEEDS_AUTH"
    | "DESCRIPTOR_STALE"
    | "ADAPTER_UPDATE_REQUIRED"
    | "UNAVAILABLE";

  incompatibleDraftItems: DraftCompatibilityIssue[];
};
~~~

Target hook:

~~~ts
function useComposerAgentSurface(selection, draft) {
  const binding = resolveBinding(selection);
  const descriptor = useAgentControlDescriptor(binding.agentProfileId);

  const options = validateDefaultsAndOverrides(
    binding.defaultSessionOptions,
    selection.turnOverrides,
    descriptor.sessionOptions
  );

  const incompatibilities = compareDraftToInputSurface(
    draft,
    descriptor.inputSurface
  );

  return projectComposerSurface(
    binding,
    descriptor,
    options,
    incompatibilities
  );
}
~~~

### Agent change reducer

~~~text
onAgentSelected(nextBindingId):
  keep draft text
  keep attachment/resource tokens
  close prior native autocomplete UI

  descriptor = loadFreshDescriptor(nextBindingId)

  for each prior session option:
    if same option ID/value validates in next descriptor:
      retain only when UI visibly communicates retention
    else:
      clear; NEVER translate to a guessed equivalent

  recompute primary option buttons
  recompute slash/reference palette
  recompute attachment compatibility

  if incompatible draft exists:
    show IncompatibleDraftBanner
    disable Send
  else:
    enable Send if ordinary validation passes
~~~

The switch is transactional from the user's point of view: dependent controls replace as
one group, not one by one while the draft changes underneath them.

## Agent picker

Rows show:
- name/icon;
- Ready / Setup required / Unavailable;
- Workspace default or Coworker default indicator;
- optional sanitized native account label when safe;
- Manage agents shortcut.

The ordinary picker lists actionable installed/bound agents, not every registry entry.
Uninstalled agents live in Agent Registry.

Selecting an agent does not silently start a session. It sets the next-turn selection and
loads the descriptor needed to render dependent controls.

## Model and reasoning/session-option controls

Never hardcode OpenAI/Anthropic/Groq model names globally.

The adapter's semantic hint controls preferred placement:
- MODEL -> Model button;
- REASONING / EFFORT -> next primary control;
- MODE -> primary only when high-value/bounded;
- OTHER -> Agent options overflow unless module UI metadata elevates it.

If an agent exposes no override, omit the control. If the option requires NEW_SESSION, the
UI says it applies at the next send/session boundary.

The UI never says an enum value exists because another agent exposed it.

## Native slash/reference behavior

~~~ts
function onComposerInput(text, caret, descriptor) {
  const trigger = nativeTriggerAtCaret(
    text,
    caret,
    descriptor.inputSurface
  );

  if (trigger.kind === "SLASH") {
    return showSlashPalette(
      descriptor.inputSurface.slashCommands,
      trigger.query
    );
  }

  if (trigger.kind === "REFERENCE") {
    return showReferencePalette(
      descriptor.inputSurface.referenceKinds,
      trigger
    );
  }

  return closeNativePalette();
}
~~~

Selecting an entry creates a semantic native input token. It is not converted into a
LiteCowork host command.

Host actions behind Plus and native agent commands are different namespaces and must stay
visually distinct even when both use familiar symbols.

## Settings > Agent Registry component tree

~~~text
AgentRegistryPage
  RegistryRail
    Search
    Filters?
    AgentRegistryRow[]
    RefreshRegistryButton

  AgentDetailPanel
    AgentIdentityHeader
    InstallationCard
      InstallButton?
      UpdateButton?
      RepairButton?
    AuthenticationCard
      AuthMethodButtons[]
      SecureCredentialSlotEditor[]
      NativeAccountObservation
    NativeConfigurationCard
      NativeConfigTargetButton[]
      NonSecretAdapterField[]
    SessionOptionsCard
      Model/options schema preview
    NativeInputCard
      input types
      slash commands
      reference kinds
    NativeExtensionsCard
      native harness summary
      LiteSPM bridge summary
    WorkspaceBindingsCard
      binding rows
      enabled/lead/default
      configure binding
    DiagnosticsCard
      descriptor/module/harness/protocol versions
      observed/expiry
      compatibility failures
~~~

## Registry rail state derivation

~~~ts
function registryRowState(
  entry,
  installation,
  descriptor,
  bindings
) {
  if (entry.adapterSupport === "NO_ADAPTER") return "NO_ADAPTER";
  if (!installation || installation.state === "MISSING") return "NOT_INSTALLED";

  if (installation.state === "INSTALLING") return "INSTALLING";
  if (installation.state === "UPDATING") return "UPDATING";
  if (installation.state === "BROKEN") return "REPAIR";
  if (installation.state === "UPDATE_AVAILABLE" && !descriptor) {
    return "UPDATE_AVAILABLE";
  }

  if (descriptor?.moduleCompatibility === false) {
    return "ADAPTER_UPDATE_REQUIRED";
  }

  if (
    descriptor?.auth.state === "NEEDS_AUTH" ||
    descriptor?.auth.state === "EXPIRED" ||
    descriptor?.auth.state === "ERROR"
  ) {
    return "NEEDS_SETUP";
  }

  if (
    descriptorIsFresh(descriptor) &&
    bindings.some(isEnabledUsableBinding)
  ) {
    return "READY";
  }

  return "INSTALLED";
}
~~~

Never derive Ready from registry listing, package presence or successful version output.

## Per-agent detail sections

### Identity

Show:
- display name;
- registry/source label;
- AgentModule ID + module version;
- descriptor API version;
- harness version;
- protocol/version;
- support/compatibility statement.

Do not show private executable paths or socket URLs.

### Installation/update

Primary state/actions:
- Not installed -> Install
- Installing -> progress/status, Cancel only if lifecycle adapter supports safe cancel
- Installed -> Check updates
- Update available -> Update
- Updating -> current lifecycle operation
- Broken -> Repair / Reinstall

A successful installation does **not**:
- sign in;
- create an AgentBinding;
- enable the binding;
- set Workspace default;
- start an AgentSession.

Lifecycle progress is recovered through operation ID + actual post-operation observation,
not only client-side percent.

### Authentication

#### Native/external auth

For NATIVE_FLOW, EXTERNAL_HANDOFF, DEVICE_CODE or NATIVE_CONFIG_ONLY, render a single
adapter-method action.

No generic email/password UI.

Examples of what the adapter may choose:
- Open native sign-in
- Continue in browser
- Show device code
- Open provider settings in native harness

#### Secure API-key slot

Only render when descriptor says:
- method kind SECRET_SLOT
- slot storage owner LITECOWORK_SECRET_STORE.

The field:
- uses password semantics;
- is never prefilled;
- is excluded from analytics/debug snapshots;
- submits once through local privileged IPC;
- clears immediately after submit;
- then displays only Configured / Needs replacement;
- never reveals SecretRef or bytes.

Multiple agent/provider keys are multiple **agent-declared slots**, not a universal
Providers list.

### Native configuration

Render only adapter-known NativeConfigTarget IDs:
- native settings UI;
- known config file;
- known config directory;
- external native UI.

The UI cannot take an arbitrary path or shell command for Open native config.

### Session/model options

Render current closed option schema with the agent's own labels.

Per option show:
- display label;
- current/default value;
- semantic hint;
- LIVE_SESSION / NEW_SESSION / UNSUPPORTED;
- source descriptor freshness.

Editing Workspace defaults uses AgentBinding configuration. A one-turn override stays at
Conversation selection/session level.

### Native input surface

Show support matrix for:
- text;
- images;
- files;
- resources;
- slash commands;
- reference/mention triggers.

Large command catalogs open a searchable panel rather than dumping hundreds of rows into
the detail screen.

### Native extensions vs LiteSPM

Show two distinct groups:

**Native to <Agent>**
- observed native MCP/plugins/hooks/skills/subagents when safe/available;
- informational unless separately qualified for mediation.

**Managed by LiteSPM**
- package/capability;
- exact compatibility route for this agent:
  - Native attachment qualified
  - LiteCowork Gateway
  - Context-only
  - Unsupported.

Do not duplicate LiteSPM package install/update forms in the agent panel. Link to
Discover/LiteSPM ownership.

### Workspace bindings

One installed/authenticated profile may have bindings for multiple Workspaces.

Each row shows:
- Workspace;
- Enabled;
- Can lead;
- Default;
- runtime/placement summary;
- descriptor/config freshness;
- Configure;
- Disable.

Configure opens:
- adapter-declared non-secret fields;
- secure credential slot references/configured status;
- default session options.

It never mirrors native provider config.

## Setup drift UX

Any of these can invalidate future admission:
- agent externally updated;
- native config changes;
- native sign-out/expiry;
- model removed;
- adapter module outdated;
- descriptor expired.

Composer behavior:
1. preserve the complete draft;
2. mark Agent control Needs setup;
3. block Send only when admission cannot be truthful;
4. show exact reason;
5. Open Agent Registry -> exact selected agent;
6. after repair/refresh, return to original conversation/draft.

No silent agent/model fallback.

## Local models

Settings > Local models is **inventory**, not routing.

A local endpoint/model becomes selectable only if the selected AgentModule projects it as
a valid model/session option. Example: an agent with native Ollama/provider support may
show a local model; another agent may show none.

## Required UI tests

1. Agent switch changes dependent controls and preserves draft.
2. Unsupported attachments block Send without deletion.
3. Model option from Agent A never survives Agent B unless B validates it.
4. slash and @ palettes change by selected Agent.
5. setup drift preserves draft and opens exact Agent Registry detail.
6. registry listing alone never displays Ready.
7. install success does not imply signed-in/enabled/default.
8. secret slot never re-renders entered key and never enters logs.
9. navigation away/back preserves lifecycle-operation truth.
10. native-config action cannot be given arbitrary path.
11. no Providers route/screen exists.
12. Local Models remains separate inventory, not a universal router.
13. LiteSPM managed vs native extensions are visually distinct.
14. keyboard focus remains in composer through dependent-control refresh.
15. screen reader announces setup errors and dependent option changes without reading
    credential values.


## Connector/workflow/learning refinements — final 2026-10-10

### New menu instead of Cowork versus Code

Do not add a persistent Cowork/Code segmented control. Use one lightweight New menu:
Chat, Code task, Research, Work with files, Automation. Each preset feeds the same
composer/Conversation/Task routes. Code task may reveal/prefill repository, worktree and
verification controls; it does not create another history/store.

### Connection detail

Connection detail shows provider/package and exact qualified revision, account display
hint, credential owner (On this device or explicit backend), provider scopes, Coworker
assignment, event mode, health and actually supported Test/Reconnect/Disconnect/Revoke.
OAuth launch/callback is provider/host-owned. No credential field is embedded in chat.

### Workflow backend

A Workflow backend Connection shows searchable existing workflows and bounded input/effect
descriptions. LiteCowork does not embed another product's visual node editor. Production
workflow use still occurs through admitted Task/Automation; settings health tests cannot
fire consequential workflows.

### Automation run detail

Run detail includes occurrence plus Task state and may show Continue, Run again and View
activity. Continue creates/opens the Task's unique follow-up Conversation. Its first host
card shows trigger, pinned Routine/Automation, outputs/evidence and unresolved items. Never
synthesize previous assistant chat bubbles from Task logs.

### Learning UX

Memory & Knowledge separates actual memory records from searchable knowledge. Learning
review appears only when candidates require action. Quiet memory has an inspectable notice;
behavioral changes show exact source/diff/test before approval. Feedback controls create
feedback/candidates only; they do not immediately mutate permissions or schedules.

### Activity and Terminal

Default Activity describes meaningful user-level work and verification. Raw terminal/tool
details are advanced inspection. Terminal is never required for normal LiteCowork use.

## V1 detailed design integration — 2026-10-12

The final implementation contract is expanded by ../docs/DESKTOP-SHELL-LLD.md (full Settings tree, left/right geometry, focus, resize and motion), ../docs/NATIVE-COMPOSER-LLD.md (agent native / @ model/effort/attachments), ../docs/CHROME-BROWSER-LLD.md (true Chrome provider/profile/attach/takeover) and ../docs/RENDERING-CONFORMANCE-LLD.md. Candidate UI components/events are not proof of implementation. Use V1-DESIGN-COVERAGE.md for scenario IDs and gate owners.

## Usage and health, rich agent cards, memory and PTY terminal — V1 UI targets

The screenshot-informed target surfaces are specified in docs/V1-TELEMETRY-USAGE-RESILIENCE-LLD.md (1D/7D/30D/90D/YTD/All/Custom + source/cost coverage), docs/V1-AGENT-DISCOVERY-AND-CATALOG-LLD.md (logo/version/detected/configured/auth/ready cards and manual-path fallback), docs/V1-AGENT-MEMORY-FAST-PATH-LLD.md (search/add scoped Memory + provenance and agent tool timing) and docs/V1-INTEGRATED-TERMINAL-LLD.md (Workbench multi-PTY profile UI including PowerShell, Bash, WSL etc). TLM-001–048 in implementation/V1-OBSERVABILITY-AGENT-MEMORY-TERMINAL-UAT.md define negative and OS/user qualification. These targets are not asserted to be implemented.

## Current desktop UI implementation audit — 2026-10-22

**Method.** Full read of `apps/litecowork-ui/src` (150 files) and
`apps/litecowork-ui/src-tauri/src` (30 files) against [EXPERIENCE](../docs/EXPERIENCE.md),
[DESIGN-SYSTEM](../docs/DESIGN-SYSTEM.md), [MOTION](../docs/MOTION.md),
[PRESENTATION-RUNTIME](../docs/PRESENTATION-RUNTIME.md), [RICH-RESPONSE](../docs/RICH-RESPONSE.md)
and [V1-UI-SCREEN-CONTRACT](../docs/V1-UI-SCREEN-CONTRACT.md), plus every `invoke` name the
bridge actually registers. Baseline evidence: `node --experimental-transform-types --test
--test-concurrency=1 "tests/*.test.ts" "tests/*.test.mjs"` → **497/497 pass**. This section
records the implementation delta only. Every normative sentence in this file and in the
contract docs stays binding; nothing below relaxes it, and nothing below claims a row is
implemented that is not.

**Tally.** 48 audited contract claims: **18 implemented/clean, 15 partial, 15 absent**; plus
3 screens implemented that no contract asks for (Goals, Ideas, Routines). Severities below
use P0 product-blocking, P1 contract violation, P2 polish.

### Implemented and honest today (do not regress)

- Shell: string-state navigation over 12 pages with `aria-current`, labelled drawer with
  focus trap and Escape (`src/App.tsx:124`, `:473–515`, `:2036`).
- Conversation catalog: real list/load/empty/error/offline/reconnect states and a durable
  create journal with failure recovery (`src/conversations/ConversationsPage.tsx:483`,
  `:514–521`, `:539–568`).
- Presentation client: transient-turn reducer with sequence-gap → `RECONNECTING`, retry
  fence, settle-to-`INCOMPLETE`, snapshot-wins-on-projection
  (`src/presentation/presentation-stream.ts:194–314`), 17-kind render registry with
  explicit fallback and rejected-item disclosure (`src/presentation/PresentationRuntime.tsx:46–103`),
  digest-validated rich fetch with semantic fallback (`ConversationsPage.tsx:64–112`).
- Status honesty: no false "connected" claims — pills gate on real `operatorReady`/stream
  `LIVE` with explicit caveats (`src/App.tsx:2977`, `:3020`; `Agents/AgentCatalogSettings.tsx:791–798`);
  failed reads never render as empty (`src/App.tsx:2898`, `:3845`; `ConversationsPage.tsx:552`).
- Notifications: real delivery history + preferences over the live bridge with transport
  -acknowledgement-only wording (`src/notifications/NotificationsPage.tsx:267–408`).
- Needs You approvals and UserRequest answer/sign-in/decline flows
  (`src/needs-you/ApprovalDetail.tsx:164`, `src/needs-you/UserRequestDetail.tsx:312–343`).
- Coworker roster/create/edit/pause/archive/primary with conflict handling
  (`src/coworkers/CoworkerSettings.tsx:492–667`).
- Library/Artifact workbench: search, roots, preview, revision editor, diff, publish,
  alertdialog with focus loop (`src/resources/library-navigation.tsx:158–183`,
  `src/artifacts/ArtifactWorkbench.tsx:516–561`).

### P0 — must still be implemented (product-blocking)

1. **Real native turn execution, end to end.** `Send` is unconditionally `disabled` with no
   handler (`src/conversations/ConversationsPage.tsx:583`) and the composer surface is
   constructed with `descriptor: null` and an empty binding so it always derives
   `NEEDS_REFRESH` (`ConversationsPage.tsx:162–174`). No turn/message `invoke` exists in
   `src-tauri/src`, and no Stop or Retry control exists anywhere. Required: daemon
   conversation-scope AgentSession admission + turn orchestration against the existing
   `CodexAppServer`/`OpenCodeServer` transports, the OpenAPI-promised `202` receipt, a Tauri
   `submit_conversation_turn`/`stop_conversation_turn` pair, then a Send/Stop/Retry composer
   driven by real state with a visible reason whenever it is disabled. Owners: E03-S01/S02.
   Until then the current honest copy (`ConversationsPage.tsx:571`,
   `first-use-guidance.ts:42–48`) must stay.
2. **Composer agent/model/effort controls wired to a real descriptor.** `AgentControlDescriptorPanel`
   and `agent-control-api.ts` exist but are referenced only by themselves — no mounted
   surface renders the agent selector, its model, its effort/mode, `+`, or slash/@ discovery
   the contract requires (`../docs/V1-UI-SCREEN-CONTRACT.md:26`). Owner: E03-S01/S06.

### P1 — contract violations still to implement

3. **Discover is not a surface.** `src/discover/DiscoverPage.tsx` is fully written but never
   mounted: absent from the `Page` union (`src/App.tsx:124`) and the nav array
   (`App.tsx:334–347`), and its row action button has no `onClick`
   (`src/discover/DiscoverPage.tsx:113`). Required by `V1-UI-SCREEN-CONTRACT.md:18` and
   `EXPERIENCE.md:14`. Owner: E04-S02/S06, E10-S01.
4. **Needs You pending count.** Nav item carries no badge (`App.tsx:338`); the projection is
   fetched but only rendered in-page (`src/needs-you/TaskAttentionPage.tsx:274`). Contract:
   sidebar count with the exact pending item, and never an invented zero
   (`DESIGN-SYSTEM.md:267–268`, action-state ledger rule). Owner: E02-S04.
5. **Task detail tabs and controls.** Task detail is one stack (`App.tsx:2824–2899`), no
   Overview/Plan/Activity/Files/Workers/Inspector tabs, and no pause/resume/steer/cancel/
   recover/handoff controls at all (`V1-UI-SCREEN-CONTRACT.md:34–36`). Those controls are
   also unserved daemon routes, so the UI must either serve them or keep them absent with an
   explanatory note — it must never render an inert button. Owner: E03-S03/S04.
6. **Right Workbench shell (Browser/Computer/Terminal/Files/Artifact).** No shell, no right
   panel, no takeover/return, no PTY surface; only the inline `ArtifactWorkbench` inside
   Library (`EXPERIENCE.md:253–271`, `V1-UI-SCREEN-CONTRACT.md:56`). Owner: E07-S02/S05, E08-S04/S07.
7. **Runtime stop/preview and startup policy.** Policy is read-only text (`App.tsx:3031`);
   no setter, no `stop_runtime` bridge command, no stop-preview dialog
   (`EXPERIENCE.md:485–489`). Owner: E01/E07.
8. **Settings breadth.** No Capabilities/Connections, permissions, storage/backup,
   appearance/accessibility, notification preference, usage, support or diagnostics
   sections, and no locality label ("applies locally / to the Workspace")
   (`V1-UI-SCREEN-CONTRACT.md:20`). Owner: E02-S01/S03.
9. **Design-token conformance.** `styles.css` defines only a subset of the contracted
   primitives and none of the semantic surface tokens (`--color-surface-raised/-info/
   -success/-warning/-danger(+soft)`, `--radius-panel/pill`, `--shadow-overlay`,
   `--ring-*`, `--text-*`, `--space-*`, `--font-ui/code`); feature stylesheets therefore
   hard-code divergent hex values (`notifications-page.css:6–7,19,30–34` including an
   off-palette `#5a3676`; `skill-proposals-page.css:17,42,60,88`; `routines-page.css:8`;
   `automations-page.css:70`; `rich-response.css:19`; `task-presentation.css:45`).
   Required: define the contracted tokens and migrate every hard-coded value. Owner: E02-S03.
10. **Light/dark/system theme.** No `.theme-dark`, no `prefers-color-scheme`, no switcher;
    `color-scheme: light` is hard-set (`styles.css:3`). Contract:
    `DESIGN-SYSTEM.md:23`, `:79–101`. Owner: E02-S03.
11. **Focus management.** No focus-trap implementation exists; Escape handlers and
    ad-hoc focus moves exist only in three places (`App.tsx:482`,
    `resources/library-navigation.tsx:159`, `coworkers/CoworkerSettings.tsx:137`).
    Contract: trap, Escape, and focus return for every blocking dialog
    (`DESIGN-SYSTEM.md:184`). Owner: E02-S01.
12. **Rich renderers.** Parser/renderer accept only 5 of the 13 registered model-safe kinds
    (`presentation/rich-presentation.ts:18–22`, `RichResponseView.tsx:7–25`): MediaFrame/
    Gallery, CodePanel, ChartFrame, DiagramFrame, Timeline, Checklist, DeliverableGroup and
    ArtifactCollection have no renderer. Unknown-kind fallback is already correct and must
    be preserved (`DESIGN-SYSTEM.md:342–346`). Owner: E08-S04.
13. **Motion conformance.** Two looping progress animations violate
    `MOTION.md:24–26` (`suggestions-page.css:33` 0.8s infinite;
    `automations-page.css:70` 900ms infinite) and the nav drawer uses an uncontracted
    `160ms ease` (`styles.css:420`) instead of the normative table. Required: static
    label/icon with `aria-busy`, and the normative duration/easing values.
    Owner: E02-S03.
14. **"Context used" disclosure and inline Task cards.** Neither exists in any mounted
    surface (`EXPERIENCE.md:121–138`). Owner: E08-S03/E03-S04.
15. **Global search / command palette.** The topbar actions area is an empty div
    (`App.tsx:2078–2079`); no palette, no shortcut. Required by
    `V1-UI-SCREEN-CONTRACT.md:7`. Owner: E02-S01.

### P2 — polish still owed

16. Composer growth: fixed `rows` with vertical resize instead of grow-to-6-lines-then-scroll
    and a sticky Send (`src/App.tsx:2441`, `ConversationsPage.tsx:581`, `styles.css:88`;
    `DESIGN-SYSTEM.md:179`).
17. Disabled-state styling relies on opacity alone in six stylesheets, contrary to
    `DESIGN-SYSTEM.md:212–213` (`styles.css:82` and the feature sheets listed above).
18. `aria-busy` is missing on the primary Save/Create/Archive controls
    (`DESIGN-SYSTEM.md:214`); it exists only in Suggestions/Goals/ApprovalDetail.
19. Status badge is 11px/26px with no icon vs the contracted 12px/20–24px with icon
    (`styles.css:191`; `DESIGN-SYSTEM.md:183`).
20. Metadata sizes below the contracted 12px floor (10px at `styles.css:98`, 11px at
    `:47,52,74`) and off-scale heading sizes; 44px touch targets exist only for two shell
    controls (`DESIGN-SYSTEM.md:156–157`, `:223–224`).
21. Work filter is a single raw status `<select>` instead of Active/Waiting/Scheduled/Done/All
    (`App.tsx:2499–2504`; `EXPERIENCE.md:791`).
22. Attachment block renders the literal word "Attachment" with no name or revision
    (`ConversationsPage.tsx:18`; `DESIGN-SYSTEM.md:180`).
23. Idea card actions do not match the contract (no Prepare/Accept split, no three-option
    "Remind me", no "Don't suggest this type" overflow;
    `suggestions/SuggestionsPage.tsx:514–525`; `EXPERIENCE.md:844–852`).
24. Suggestion spinners are the looping-animation case from MOTION item 13 above.

### State-coverage gaps to close per surface

- **PresentationRuntime** has no error state (`PresentationRuntime.tsx:123–124` only).
- **SkillProposalsPage** has loading/ready/empty but no error and is not mounted
  (`skills/SkillProposalsPage.tsx:13,98`).
- **No surface implements a permission-denied state**; the closest is the offline read-only
  gate (`App.tsx:2104`, `ConversationsPage.tsx:547`).
- Every other mounted screen (Conversations, Work, Library, Coworkers, Needs You,
  Automations, Routines, Notifications, Goals, Suggestions, First-run, Settings) already
  distinguishes loading/empty/error/offline and must keep doing so.

### Implemented but unclaimed by any contract

Goals (`src/goals/GoalsPage.tsx`), Ideas/Suggestions (`src/suggestions/SuggestionsPage.tsx`)
and Routines (`src/routines/RoutinesPage.tsx`) are mounted screens with no row in
`V1-UI-SCREEN-CONTRACT.md`. They are not violations, but they must not be used as evidence
of contract coverage, and their "not active in this build" copy
(`src/App.tsx:2152`, `AutomationsPage.tsx:238`) must stay truthful.

### Explicit non-claims

- No native Conversation turn is executed by this build; the streaming client is wired but
  its Operator producer does not exist, so no transient frame reaches a running app.
- No Browser/Computer/Terminal surface exists; the presentation Browser card is informational
  only (`PresentationRuntime.tsx:59`).
- Worker profiles cannot be enabled for execution (`AgentCatalogSettings.tsx:652`) and the
  agent catalog is display-only for authentication/inference/model selection
  (`App.tsx:818`).
- The Discover and SkillProposal components exist in source but are unreachable surfaces.
