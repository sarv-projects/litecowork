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
| Global Automations | Cross-Coworker canonical list | Filter, new, edit, pause, run now, view trigger/occurrence | invalid scope, missed slots, duplicate run, host offline | E09 |

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
