# V1 Desktop — Final Screen and Interaction Contract

**Status:** normative UX TARGET, not evidence of installed build; subordinate to [Product](V1-PRODUCT-RELEASE-CONTRACT.md), [Experience](EXPERIENCE.md), [Desktop Shell](DESKTOP-SHELL-LLD.md), [Native Composer](NATIVE-COMPOSER-LLD.md), [Design System](DESIGN-SYSTEM.md), [Accessibility cases](../implementation/UI.md).

**Implementation status (2026-10-22).** Every row below is binding target; the per-surface
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

**App shell:** persistent left sidebar (collapsible and resize-aware), central Conversation, optional context-sensitive right Workbench. Do not add a separate generic global model selector or mandatory Coworker/Task wizard. Per Workspace remember layout only, not expired authority, native sessions or grants. Keyboard palette and global search operate on authorized persisted data.

| Surface | Navigation | Essential user actions / availability rules |
|---|---|---|
| New Chat | First-class prominent action | Create ordinary Conversation with selected supported native AgentBinding; preserve unsent draft if unavailable; default Workbench closed |
| Recent Chats & Search | Sidebar under New Chat | Full text/metadata search over eligible committed history; filter; rename, pin, archive/delete; draft and streaming text distinguished from durable history |
| Coworkers | Sidebar | Roster, Quick Create, Customize, last chat, scoped memories, connections, workers and approved responsibilities; no automatic permission escalation |
| Needs You | Sidebar with actual count only | Approval, UserRequest, authentic provider sign-in, conflict, takeover and recovery with exact target, expiry and owner |
| Work | Sidebar | Task list and details: outcome, live status, Steps/Attempts, meaningful activity, controls, blockers, evidence, verification and worker handoff |
| Automations | Sidebar | Routine and schedule builder, timezone/DST, active host/dependencies, run history and missed occurrences, pause/disable/test |
| Library | Sidebar | Files/artifacts by Workspace; preview, edit if supported, compare revisions, Save As, export, source evidence and task/conversation backlink |
| Discover | Sidebar | Catalog + existing host inventory + Connections; host compatibility, install/adopt/copy/repair/update/unmanage and safe operation previews |
| Agents | Settings or Context/Inspector | Installed/detected qualified harnesses, native auth/models/modes/config, native subagents and provider errors; never claim unsupported native parity |
| Settings | Sidebar footer | Workspace, Agents, Capabilities/Connections, permissions, storage/backup, Coworker defaults, appearance/accessibility, Runtime, notifications, usage, support and diagnostics |

On desktop, Workbench may dock right, undock into a supported window, collapse, or be hidden. On narrow widths make Workbench a mutually exclusive overlay/panel with focus restoration; sidebar becomes a drawer. Use real keyboard interaction and focus traps for blocking dialogs. Never cram both desktop sidebars into a phone-sized viewport.

## Universal conversation

Composer elements: file/reference add; multiline input; Send/Stop; native **Agent selector → that agent's valid Model/options controls**; optional native effort/mode, slash commands and @ references discovered from a qualified descriptor. Display agent identity clearly without imposing global model/provider ontology. Basic Send never creates Task/Coworker. Allow explicit contextual **Run as Task**, **Schedule** and **Save as Routine** actions; these open reviewed flows, not a row of permanent mode pills.

Message rendering: Markdown/code, citations to eligible source, attachments, native tool status, real streamed semantic fragments and optional rich presentation. Do not persist a frame as durable message until committed by owner. Show partial/reconnecting/failed states and safe retry on same durable identity. Native steps/tool events may be reported/observed, not necessarily mediated.

Conversation switch: if no active work, select a different qualified agent and begin a compatible new native session while keeping human conversation. During durable Task execution, route switch through accepted lead-assignment/handoff, fence active Attempts; never transplant native private provider history.

## Work/Tasks and Needs You

Work list defaults to outcome/state/last activity and an actionable blocker, not raw logs. Task detail supports tabs Overview, Plan, Activity, Files/Artifacts, Workers and Inspector. Native planning remains lead-owned, reviewed/accepted Steps are durable and pin versions. Show request for exact approval with operation, target, identity, scope, time expiry and consequences. No invented progress percentage, cost or quota.

Task controls must be backed by actual served commands and owner state: pause, resume, steer, cancel, recover, handoff. If not qualified, disable with specific reason rather than presenting an inert button.

## LiteSPM discovery and management UX

Distinct filters or tabs **Explore**, **Already installed/native**, **Managed**, **Connections**. Keep two independent concepts visible:
- **Installation origin/status:** observed external, managed, adopted, drifted, broken, unknown.
- **Use readiness:** compatible host, installed, authenticated, Coworker assigned, granted, activated, invoked.

**Preview-before-mutation** modal: exact host, user/project scope, named native config path (redacted where sensitive), operation, preimage diff, package source, conflicts, permissions and reversible changes. Show `Manage existing` only if LiteSPM offers a qualified adopt method for the exact kind. `Stop managing` cannot become `uninstall` for an originally existing native entry. Mutation reports real receipt or UNKNOWN reconciliation.

Empty state differentiates no catalog entries, no detected installations, unsupported agent, offline inventory, blocked package service and insufficient auth. Do not replace unavailable with zero results.

## Coworkers, memory and responsibility

Quick Create: name and intent, inherited default lead, optional initial chat; setup takes a few steps and never auto-enables a schedule/connection. Customize: identity, optional native chief options, scoped context, optional workers, connections, memory policy, responsibilities as drafts, final review. Creation and first chat are separate durable operations with separate idempotency keys, safe retry after partial failure.

Coworker view: chat first; Overview, Memory/Knowledge, Tools & Connections, Workers, Routines, Activity and Settings. Memory entries show provenance, scope, conflicts, last confirmation, revoke/purge receipt. Private learning may operate quietly only under eligible accepted policy; no self-granting or self-publishing skill.

## Workbench and formats

Right-panel tabs or views: Activity, Browser, Computer, Terminal, Files, Artifact Preview/Edit. Source-of-truth is the owning Runtime/environment/file/artifact service, not UI ephemeral state. Human takeover must prove a fresh control epoch and return must revalidate authority. Terminal is real PTY or explicitly unavailable; no simulated shell output. Browsing through an embedded WebView does not imply authorized automation.

| Artifact type | Capability required to claim support |
|---|---|
| Text/Markdown | Semantic preview, editing, save, version and export |
| Code/project | Tree, diffs, save/revert and explicit workspace/provenance |
| PDF | Accurate preview/search/export; annotate only if a real qualified writer exists |
| DOCX/XLSX/PPTX | Per-format generation, preview, editing/round-trip, version and export; disclose partial provider support |
| Image | Preview, exact source/provenance, supported edit or view-only and export |
| Interactive view | Validated presentation, keyboard, safe callbacks and semantic fallback |

## Automations and background

Wizard: start with request → explicit Routine draft → schedule/event condition → timezone/misfire/overlap → dependencies/local host/authorization → review and enable. Runs page links occurrence to Task/outcome, errors and notifications. Closing UI does not claim 24/7 service: show when Runtime is alive, OS locked, computer asleep, provider offline or site unavailable. No silent schedule inferred from casual phrasing.

## Global states and accessibility

Every surface tests loading, empty, ready, stale, unauthorized, needs-auth, unsupported, offline, error, concurrent edit, revoked and partial provider result. Every actionable button must have a handler and a visible confirmation/error; real focus/keyboard/screen-reader names; WCAG-appropriate contrast; reduced motion; responsive text/zoom; no screen-spoofing of fake “active” providers.

## Release screen acceptance

Qualify a cold install and one practical end-to-end scenario per screen on the actual desktop package, exact OS and real provider. Pixel-quality review is separate from functional control qualification. A prototype screenshot or standalone HTML does not count as native integration.
