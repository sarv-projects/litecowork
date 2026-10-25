# V1 Desktop — Final Screen and Interaction Contract

**Status:** normative UX TARGET, not evidence of installed build; subordinate to [Product](V1-PRODUCT-RELEASE-CONTRACT.md), [Experience](EXPERIENCE.md), [Desktop Shell](DESKTOP-SHELL-LLD.md), [Native Composer](NATIVE-COMPOSER-LLD.md), [Design System](DESIGN-SYSTEM.md), [Accessibility cases](../implementation/UI.md). The owner-approved screenshot layout (2026-10-24) supersedes prior closed-by-default wording; the right Workbench is visible by default on desktop.

**Current implementation correction (2026-10-25).** The shared shell now mounts a compact Start Workbench by default on desktop. Files switches the Workbench to loaded Workspace resources, cursor paging and an exact-revision text preview; a secondary action opens the full Library. Work and Runtime & settings route to existing screens. Browser, Terminal and Computer are visibly unavailable because no qualified providers are connected. Desktop collapse preference is stored per Workspace; narrow widths use a modal drawer with focus trapping and restoration. Returning installs now open the Chats route by default; first-run Workspace setup remains available through Settings. The task composer remains accessible as **New task**. New chat creates a blank local Conversation and may carry an existing first-use draft into its window-only composer; live chat send/session admission is not implemented. This note supersedes the older Workbench source-gap statement below; the screen contract remains the target.

**Chats source correction (2026-10-25).** The sidebar route is labeled **Recent chats** and opens the saved local conversation workspace. Its first-class **New chat** action creates a blank Conversation through the existing idempotent create and recovery path. When creating the first Conversation while a first-use draft exists, the draft is carried into that Conversation as window-only composer text and stays unsent. The Send control remains disabled because native agent session admission and turn execution are not wired. New chat is disabled until a Workspace is selected, the Local Operator is ready, and pending create requests have been checked.

**Current source correction (2026-10-24).** Discover is mounted in the sidebar with **Explore**, **Installed / native**, **Managed**, and **Connections** sections. The local read route currently returns Workspace-local skill records and Runtime-observed capability hosts only; Explore and Installed / native are labeled to match those exact sources, and skill results use the route's opaque cursor for paging. The runtime-observed list is bounded to at most 50 hosts and explicitly is not a complete native inventory. Managed-package and Connections sections explain that their owning routes are unavailable; no empty inventory, install, auth, or ownership claim is inferred. Per-skill capability details load only after the owner opens the disclosure. Local skill execution and package Connect/Use actions remain disabled with source-specific explanations. Closing details suppresses late rendering; an already-dispatched Operator request may finish. The 2026-10-22 implementation snapshot below is historical; this note supersedes its Discover clause.

**Implementation status snapshot (2026-10-22; historical).** Every row below is binding target; the per-surface
file-cited delta lives in the UI implementation audit in
[`implementation/UI.md`](../implementation/UI.md). Rows that are wholly unmet today, with
the exact reason: **New Chat send** — the turn route always returns 503 and the composer
renders a disabled Send; **Recent Chats rename/pin/archive/delete** — no conversation
mutate route exists; **Coworker scoped memories** — the memory routes are explicitly not
implemented; **Work controls/evidence** — the control and effects routes are target-only
and the UI shows no controls (and therefore no inert buttons either); **Task detail tabs**
— the detail is one read-only stack; **Discover** — the page component exists but is not
mounted and install/adopt/Connections routes are target-only; **Workbench Browser/
Computer/Terminal** — no shell and no provider routes; **PDF and Office artifact rows** —
no generation or preview capability is served; **composer Run as Task / Schedule / Save as
Routine** — no such actions exist; **streamed semantic fragments** — the Operator emits no
transient frames; **global search and keyboard palette** — no route and no UI.
Met today: Automations wizard and run history, Needs You (list, not count badge), Library,
Notifications, and the global state matrix on the surfaces that exist.

## Shell and navigation

**App shell:** persistent left sidebar (collapsible and resize-aware), central Conversation, visible compact right Workbench. The default start state shows Browser, Terminal and File entry tiles, with actual availability labels; opening a tile changes the content of this same right-side surface. Do not add a separate generic global model selector or mandatory Coworker/Task wizard. Per Workspace remember layout only, not expired authority, native sessions or grants. Keyboard palette and global search operate on authorized persisted data.

| Surface | Navigation | Essential user actions / availability rules |
|---|---|---|
| New Chat | First-class prominent action | Create ordinary Conversation with selected supported native AgentBinding; preserve unsent draft if unavailable; show compact Workbench by default on desktop |
| Recent Chats & Search | Sidebar under New Chat | Full text/metadata search over eligible committed history; filter; rename, pin, archive/delete; draft and streaming text distinguished from durable history |
| Coworkers | Sidebar | Roster, Quick Create, Customize, last chat, scoped memories, connections, workers and approved responsibilities; no automatic permission escalation |
| Needs You | Sidebar with actual count only | Approval, UserRequest, authentic provider sign-in, conflict, takeover and recovery with exact target, expiry and owner |
| Work | Sidebar | Task list and details: outcome, live status, Steps/Attempts, meaningful activity, controls, blockers, evidence, verification and worker handoff |
| Automations | Sidebar | Browse coding and general-cowork starter templates; preview requirements and outputs; create an editable Routine draft; then builder, timezone/DST, active host/dependencies, run history and missed occurrences, pause/disable/test. A template never installs, connects, grants, enables, or runs |
| Library | Sidebar | Files/artifacts by Workspace; preview, edit if supported, compare revisions, Save As, export, source evidence and task/conversation backlink |
| Discover | Sidebar | Catalog + existing host inventory + Connections; host compatibility, install/adopt/copy/repair/update/unmanage and safe operation previews |
| Agents | Settings or Context/Inspector | Installed/detected qualified harnesses, native auth/models/modes/config, native subagents and provider errors; never claim unsupported native parity |
| Settings | Sidebar footer | Workspace, Agents, Subagents (Workspace default and Coworker inherit/narrow override), Capabilities/Connections, permissions, storage/backup, appearance/accessibility, Runtime, notifications, usage, support and diagnostics |

On desktop, Workbench may dock right, undock into a supported window, collapse, or be hidden. On narrow widths make Workbench a mutually exclusive overlay/panel with focus restoration; sidebar becomes a drawer. Use real keyboard interaction and focus traps for blocking dialogs. Never cram both desktop sidebars into a phone-sized viewport.

## Universal conversation

Composer elements: file/reference add; multiline input; Send/Stop; native **Agent selector → that agent's valid Model/options controls**; optional native effort/mode, slash commands and @ references discovered from a qualified descriptor. Display agent identity clearly without imposing global model/provider ontology. Basic Send never creates Task/Coworker. Allow explicit contextual **Run as Task**, **Schedule** and **Save as Routine** actions; these open reviewed flows, not a row of permanent mode pills. Show a Conversation-scoped usage affordance: switching agents does not reset the chat total; drill-down attributes observations to agent, turn, provider and model. Keep provider-billed, agent/gateway-reported, and estimated costs separate; group by currency; label incomplete/opaque usage as unknown rather than $0.00.

Message rendering: Markdown/code, citations to eligible source, attachments, native tool status, real streamed semantic fragments and optional rich presentation. Do not persist a frame as durable message until committed by owner. Show partial/reconnecting/failed states and safe retry on same durable identity. Native steps/tool events may be reported/observed, not necessarily mediated.

Conversation switch: keep the same Conversation ID, messages, draft, attachments, and pinned Resources. A different selected agent handles the next turn and receives the committed history prefix through that turn, with source-agent attribution. A turn already running remains on its admitted agent. The adapter may start a new provider AgentSession internally; never transfer provider-private session history or credentials. If context compaction is needed, show the source range/summary and preserve the full saved transcript. Costs from each distinct measured agent inference accumulate under this same Conversation, with duplicate observations reconciled, billed/reported/estimated lanes kept separate, and currencies never silently combined. During durable Task execution, use the accepted lead-assignment/handoff and fence active Attempts; do not reinterpret a chat switch as Task handoff.

## Work/Tasks and Needs You

Work list defaults to outcome/state/last activity and an actionable blocker, not raw logs. Task detail supports tabs Overview, Plan, Activity, Files/Artifacts, Workers and Inspector. Native planning remains lead-owned, reviewed/accepted Steps are durable and pin versions. Show request for exact approval with operation, target, identity, scope, time expiry and consequences. No invented progress percentage, cost or quota.

Task controls must be backed by actual served commands and owner state: pause, resume, steer, cancel, recover, handoff. If not qualified, disable with specific reason rather than presenting an inert button.

## LiteSPM discovery and management UX

Present the package workflow inside LiteCowork's first-party Discover surface; LiteSPM remains the authority for catalog, host compatibility, installation, ownership ledger and rollback. Do not iframe the public catalog or treat its website HTML as an API. Use a published, version-pinned LiteSPM bridge/CLI contract for data and mutations; until its exact local protocol is qualified, show inventory that can be read honestly and keep unsupported install actions unavailable.

Distinct filters or tabs **Explore**, **Already installed/native**, **Managed**, **Connections**. Package types are **MCP servers**, **Skills**, and **Plugins**, with a package-level component tree for plugins. Every result explicitly labels **installable**, **discovery-only**, **unsupported for this host**, or **unavailable/unknown**; a catalog listing never implies installability. Show source/publisher and pinned revision, package digest when available, host target(s), user/project scope, compatibility evidence, requested permissions, dependencies, update/drift state and exact reason for any refusal. Counts and featured ranking are time-bounded catalog observations, not verified popularity or a promise that every listing can be installed.

The owner selects one or more supported target agents and scope. LiteSPM produces a read-only plan first: exact components, exact host/config destinations, structural changes, preimage/fingerprint, missing prerequisites, secret names only, risk, reversibility, conflicts and expiry. A human reviews and approves each mutation; an agent can search and propose a plan but cannot self-approve it. The plan cannot grant a LiteCowork capability or authenticate an external provider. Plugins expand into all declared MCP/Skill components before approval; no hidden partial install is implied. Preserve managed, observed and adopted ownership separately, and reconcile an ambiguous outcome before retry.

Once installed, guide an eligible provider/Connection sign-in as a separate action. Show **Installed**, **Authenticated**, **Assigned**, **Granted**, **Activated** and **Invoked** as different states; remove/unmanage controls must explain whether they remove LiteSPM-owned entries or leave pre-existing native entries untouched.

**Preview-before-mutation** modal: exact host, user/project scope, named native config path (redacted where sensitive), operation, preimage diff, package source, conflicts, permissions and reversible changes. Show `Manage existing` only if LiteSPM offers a qualified adopt method for the exact kind. `Stop managing` cannot become `uninstall` for an originally existing native entry. Mutation reports real receipt or UNKNOWN reconciliation.

Empty state differentiates no catalog entries, no detected installations, unsupported agent, offline inventory, blocked package service and insufficient auth. Do not replace unavailable with zero results.

## Coworkers, memory and responsibility

Quick Create: name and intent, inherited default lead, optional initial chat; setup takes a few steps and never auto-enables a schedule/connection. Customize: identity, optional native chief options, scoped context, optional workers, connections, memory policy, responsibilities as drafts, final review. Creation and first chat are separate durable operations with separate idempotency keys, safe retry after partial failure.

Coworker view: chat first; Overview, Memory/Knowledge, Tools & Connections, Workers, Routines, Activity and Settings. Memory entries show provenance, scope, conflicts, last confirmation, revoke/purge receipt. Private learning may operate quietly only under eligible accepted policy; no self-granting or self-publishing skill.

## Workbench and formats

Right-panel tabs or views: Activity, Browser, Computer, Terminal, Files, Artifact Preview/Edit and Interactive App. The app view remains inside the same Workbench. Its inert preview shows the exact version and source; Run/Stop appears only for a qualified isolated Runtime. Source-of-truth is the owning Runtime/environment/file/artifact service, not UI ephemeral state. Human takeover must prove a fresh control epoch and return must revalidate authority. Terminal is real PTY or explicitly unavailable; no simulated shell output. Browsing through an embedded WebView does not imply authorized automation.

| Artifact type | Capability required to claim support |
|---|---|
| Text/Markdown | Semantic preview, editing, save, version and export |
| Code/project | Tree, diffs, save/revert and explicit workspace/provenance |
| PDF | Accurate preview/search/export; annotate only if a real qualified writer exists |
| DOCX/XLSX/PPTX | Per-format generation, preview, editing/round-trip, version and export; disclose partial provider support |
| Image | Preview, exact source/provenance, supported edit or view-only and export |
| Interactive app | Validated self-contained package, inert preview, exact-version Run/Stop through isolated Artifact Runtime, keyboard/accessibility, no network or Effect authority, and source/static fallback |

## Automations and background

Wizard: start with request → explicit Routine draft → schedule/event condition → timezone/misfire/overlap → dependencies/local host/authorization → review and enable. Runs page links occurrence to Task/outcome, errors and notifications. Closing UI does not claim 24/7 service: show when Runtime is alive, OS locked, computer asleep, provider offline or site unavailable. No silent schedule inferred from casual phrasing.

### Coding and Cowork starter flow

The Automations screen presents two editable starter cards: **Weekday codebase review** (Coding) and **Weekly project briefing** (Cowork). Each card discloses its prerequisites and the exact result a due schedule can create. The Coding Routine is review-only; the Cowork Routine is source-grounded. Both forbid edits, external sends, and publishing.

Choosing a card opens a prefilled, unsaved Routine editor in Routines. The user can edit or cancel it; nothing is persisted until **Save Routine**. After save, the app captures the Routine ID and immutable current revision. **Continue to Automation** opens the Automation editor with that exact Workspace-scoped revision pinned and verified. The schedule, local IANA time zone, and policy remain editable; defaults are 09:00 local time on weekdays for Coding and Mondays for Cowork. Saving creates a paused Automation. It is never enabled or run by template selection.

A due occurrence in this build can materialize one ordinary **READY Task**. It does not plan, launch an agent or subagent, edit project files, send, or publish. A saved Routine remains available if the app closes, but the in-progress cross-screen template handoff is held in app memory; after restart, the owner can resume by creating an Automation and selecting the Routine. Changing Workspace clears the unsaved handoff. Starter cards never install a capability, connect an account, grant authority, or create a Coworker.



## Global states and accessibility

Every surface tests loading, empty, ready, stale, unauthorized, needs-auth, unsupported, offline, error, concurrent edit, revoked and partial provider result. Every actionable button must have a handler and a visible confirmation/error; real focus/keyboard/screen-reader names; WCAG-appropriate contrast; reduced motion; responsive text/zoom; no screen-spoofing of fake “active” providers.

## Release screen acceptance

Qualify a cold install and one practical end-to-end scenario per screen on the actual desktop package, exact OS and real provider. Pixel-quality review is separate from functional control qualification. A prototype screenshot or standalone HTML does not count as native integration.


**Source correction update (2026-10-24).** Discover now exposes an on-demand **View capabilities** disclosure on each catalog row. It requests the workspace-scoped detail only after the owner opens it, renders capability names, operation classes and required scope names, and labels the view review-only; no capability is granted or installed. Closing the disclosure aborts the UI signal and suppresses late rendering; Tauri invoke cannot cancel the already-dispatched local Operator HTTP request, which may finish in the background. The detail may also show the item description and provider note as catalog text. Package Connect/Use remain disabled because the LiteSPM package contract is not qualified. A Workspace-local skill's Open action is disabled because Discover does not route skill execution. Install, adoption, Connections management, repair and update remain unavailable.

Implementation boundary: closing the disclosure aborts the UI signal and suppresses late rendering. Tauri invoke does not cancel the already-dispatched local Operator HTTP request; it may finish in the background.


## 2026-10-25 competitor-informed continuity addendum

Keep Chat and Task distinct by lifecycle while allowing an explicit, in-place Chat-to-Task promotion. Promotion preserves the same transcript, unsent draft, selected AgentBinding and attached ResourceRefs; it creates a durable Task only after the user chooses **Run as Task** and reviews its declared outcome/authority. Show the resulting task status, agent/worker provenance, Needs You decisions and reviewable artifacts in the existing conversation/Work surfaces. Do not force users to start a separate Coworker thread to continue longer work, and do not imply that local V1 work continues while the machine is asleep/off. A cloud Task is a different eligible placement with its own route/consent; it is not a fallback that silently receives local files.

OpenAI Dots, ChatGPT Work/Codex, Anthropic Claude/Cowork, Microsoft Copilot Cowork and xAI Grok Bot are distinct product surfaces with different local/cloud and identity boundaries. LiteCowork's Agent selector pins the native harness and model route; the Task surface pins a separate hosted-worker route only when explicitly selected and independently qualified. A visual selector or a Coworker avatar never implies cross-agent subscription access.

Evidence and updated route comparison: [Competitive Product Research](COMPETITIVE-RESEARCH.md#2026-10-25-official-product-delta--persistent-assistants-unified-cowork-desktop-control).
