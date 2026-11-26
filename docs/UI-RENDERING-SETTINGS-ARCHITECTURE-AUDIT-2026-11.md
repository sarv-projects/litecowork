# UI, agent-output rendering, Intelligent UI, Settings and architecture audit (2026-11-09)

**Status:** evidence audit. This is not an owner contract. Normative behavior stays with ARCHITECTURE.md, the owner docs and the accepted feature packages. Findings that need new work were mapped to an existing package task or listed as a proposed task addition for the owner (Section 9).
**Outcome (2026-11-09):** the owner approved folding the Section 9 proposed tasks P-00..P-09 into their owning packages, including the LOCKED packages, under the ACP direction (register [A-55](CONTRADICTION-AND-ALIGNMENT-REGISTER.md) and [A-54](CONTRADICTION-AND-ALIGNMENT-REGISTER.md), recorded in [OWNER-ACCEPTANCE-REVIEW-PACKET.md](OWNER-ACCEPTANCE-REVIEW-PACKET.md) Part 5). Those items are accepted package tasks now, not open proposals; the destination task ids are in [OWNER-ACCEPTANCE-REVIEW-PACKET.md](OWNER-ACCEPTANCE-REVIEW-PACKET.md) Part 5 and [V1-PRODUCT-FEATURE-COVERAGE.md](V1-PRODUCT-FEATURE-COVERAGE.md). The "proposed P-NN" wording below is preserved as the original proposal record.
**Revision audited:** branch `docs/audit-competitor-superset` at e678a91. Read-only source review. No build or test was run for this audit.
**Related owners:** [V1-UI-SCREEN-CONTRACT.md](V1-UI-SCREEN-CONTRACT.md), [V1-UI-ACTION-STATE-LEDGER.md](V1-UI-ACTION-STATE-LEDGER.md), [SETTINGS-AND-SHELL-SURFACE-AUDIT.md](SETTINGS-AND-SHELL-SURFACE-AUDIT.md), [V1-SETTINGS-CONTRACT-LEDGER.md](V1-SETTINGS-CONTRACT-LEDGER.md), [PRESENTATION-RUNTIME.md](PRESENTATION-RUNTIME.md), [INTELLIGENT-UI-AND-IMAGE-CONTENT.md](INTELLIGENT-UI-AND-IMAGE-CONTENT.md), [NATIVE-COMPOSER-LLD.md](NATIVE-COMPOSER-LLD.md), [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md).

## 1. Verdict

- **Agent output probably never reaches the screen on OpenCode v2.0.32.** The supervisor reads text and completion from keys that the published OpenCode v2 event envelope does not carry at the top level (R-01). If that holds on a real server, every turn fails closed after 60 s with no answer. This is the single most important finding and blocks E2E-01.
- **When text does arrive, it renders poorly.** There is no streaming producer, the Markdown renderer rejects a whole message on common syntax (tables, `snake_case`, JSON braces, `---`), code has no highlighting or copy, and tool calls, reasoning, file edits and permission prompts are discarded before storage.
- **Intelligent UI is display-only and partly unreachable.** Rust accepts 2 rich block kinds and TypeScript accepts 5, the caps differ (1 MiB vs 512 KiB), and nothing in production publishes a rich document. The declarative components in INTELLIGENT-UI (charts, forms, SAFE_CALCULATOR) and the Interactive Artifact Runtime have no source.
- **UI shell:** 13 page types exist but only 9 are reachable. Goals, Notifications and Ideas are mounted with no navigation path. Back/Forward buttons call `window.history` while navigation never pushes history. Three Conversation buttons have no handler. The Workbench Browser iframe combines `allow-scripts` with `allow-same-origin`, which cancels the sandbox.
- **Settings:** 4 of 16 target categories are mounted. All 7 General controls are inert (4 selectors that change only component state and 3 shortcut labels with no handlers). The other 12 categories are absent with no "unavailable" explanation, which breaks accepted B-29. About 19 of 187 ledger rows are wired in Settings, about 25-30 are served on other screens, and about 135 have no control anywhere.
- **Architecture** is sound at the boundaries (OS-authenticated IPC, no SQL in daemon handlers, strict CSP, no `dangerouslySetInnerHTML`), but concentrated in very large files and missing the event-normalization layer that the agent pipeline needs.
- **Cowork-likeness:** the product has the right nouns (Workspace folders, Tasks, Coworkers, Needs You, Automations, Library) but the conversation itself shows only a final text bubble. A cowork-style product is judged by what you see while the agent works: a live plan, tool and file activity, permission prompts, and deliverables. Section 8 lists the changes in order, each kept within the AGENTS.md invariants.

## 2. Agent output pipeline as built

```text
Composer Send (ConversationsPage.tsx ~748-765; content=[TEXT], resource_refs=[])
 -> Operator commits USER message + OPEN turn
 -> AgentSessionSupervisor::dispatch_turn (supervisor.rs:112, spawn_blocking)
 -> execute_agent_turn: OpenCodeServer::spawn per turn, create_session, activate (OPEN->RUNNING)
 -> GET /api/event SSE (opencode_server.rs:635-680); POST /api/session/{id}/prompt (:559, :593)
 -> loop next_event(500 ms) until a hard 60 s deadline (supervisor.rs:529-566)
      top-level data.delta string   -> append
      top-level data.content string -> replace
      SSE event "message_completed" or data.done == true -> completed
      everything else               -> dropped
 -> server.stop(); fail closed if not completed or empty (supervisor.rs:568-575)
 -> commit_conversation_agent_response: content = [{kind: TEXT, text}] (supervisor.rs:601)
 -> presentation stream: ready / projection / resync frames only (no turn.delta producer)
 -> Tauri event "conversation-presentation-stream" with per-frame ack
 -> AGENT row -> RichResponseView -> StructuredTextPreview (custom Markdown)
```

| ID | Severity | Finding | Evidence | User-visible effect |
|---|---|---|---|---|
| R-01 | Critical | The published OpenCode v2 HTTP API ([dev.opencode.ai/v2/docs/api](https://dev.opencode.ai/v2/docs/api/), the URL the module header cites) wraps every event as `{id, created, metadata, type, durable, location, data}` and names text, completion and errors `session.text.delta`, `session.text.ended`, `session.idle`, `session.execution.succeeded`, `session.execution.failed`. The supervisor reads `delta`, `content`, `done` and session ids only at the top level of the parsed JSON, and waits for an event named `message_completed`, which is not in the v2 event catalogue. | supervisor.rs:536-557; opencode_server.rs:9, :928 | Probably no text accumulates and completion is never seen, so each turn fails after 60 s with "stream completed=false". The repo has no captured v2.0.32 turn fixture; AGENT-FABRIC.md:165 records only a version and handshake observation. Confirm with a real capture before changing code. |
| R-02 | High | Hard 60 s read deadline and 500 ms poll. | supervisor.rs:529-531 | Any real agent work (tools, edits) is cut off. Already in 001-T04. |
| R-03 | High | No Rust producer for `turn.delta` or `rich.draft`, while the UI reducer supports both. | presentation-stream.ts:240-290, :371-414; PRESENTATION-RUNTIME.md:336 | No streaming text, the caret never shows, and Stop has nothing to target. Covered by 001-T13 and 001-T16. |
| R-04 | High | Every non-text event is dropped and the committed message is one TEXT block. Reasoning deltas would be concatenated into the answer if they arrived as a top-level `delta`. | supervisor.rs:547-551, :601 | Tool calls, file edits, permission requests, plan or todo updates, errors and usage are invisible. Reasoning could leak into the answer. Not covered by any task (proposed P-01, P-02). |
| R-05 | High | Markdown parsing is all-or-nothing. A table line, horizontal rule, indented code, nested list, fence with an info string, image, or any leftover `_ ~ [ ] { } *` makes the whole message fall back to raw `<pre>` with "Some Markdown syntax is unsupported". | StructuredTextPreview.tsx:92-93, :122-127, :146, :155 | Typical LLM answers (tables, `snake_case`, JSON) show as raw text. Proposed P-03. |
| R-06 | Medium | Code blocks have no language label, highlighting, copy button or wrapping toggle; the language token is parsed and discarded. | StructuredTextPreview.tsx:125-131, :201 | Code answers are hard to use. Proposed P-03. |
| R-07 | Medium | Failed turns show no inline error card in the transcript; only `turnError` from submit is shown. Supervisor errors are `format!("{e:?}")` strings. | ConversationsPage.tsx:948; supervisor.rs:564, :571 | A failed turn looks like silence. Partly 001-T04 (typed FAILED), proposed P-02 for the card. |
| R-08 | Medium | Message list is a plain `.map` with no virtualization and no stick-to-bottom autoscroll. | ConversationsPage.tsx:874, :933-942; NATIVE-COMPOSER-LLD.md:112 | Long chats slow down; new answers do not scroll into view. Partly 014-T15; proposed P-04 for autoscroll. |
| R-09 | Low | RESOURCE content renders as the word "Attachment"; user messages are plain text. | ConversationsPage.tsx:17, :879 | Attachments are not identifiable. 001-T14. |
| R-10 | OK | No `dangerouslySetInnerHTML`, `innerHTML` or `srcDoc` in `src/`; links are https-only with a confirm and `noopener,noreferrer`. | StructuredTextPreview.tsx:74-87, :184 | Keep this posture in every renderer change. |

## 3. Intelligent UI, rich presentation, Artifacts and Workbench

| Capability | Target owner | Source | Served to the UI |
|---|---|---|---|
| PresentationItem cards (TEXT, CITATION, ACTIVITY, TASK_CARD, APPROVAL, ARTIFACT, CODE, DIFF, TABLE, CHART, IMAGE, ERROR) | PRESENTATION-RUNTIME.md | PresentationRuntime.tsx:21-63; presentation_operator.rs | Yes for Task progress. CHART renders as a list and IMAGE as a placeholder. |
| RichPresentation TEXT_SLICE, LAYOUT | PRESENTATION-RUNTIME.md | Rust rich_presentations.rs:278, :306; TS rich-presentation.ts | At this audit checkpoint, display-only; T08 later added a post-settlement table-only caller to `publish_rich_presentation`. Provider structured-intent ingress remains absent. |
| RichPresentation CARD, CALLOUT, TABLE | partial target | TypeScript only (rich-presentation.ts:18-22) | No. Rust rejects them. |
| DIVIDER | storage-core rich_presentation.rs:86 doc comment | Neither validator | No. |
| Caps | specs/016 SPEC (512 KiB both sides) | Rust 1 MiB (storage-core rich_presentation.rs:6), TS 512 KiB (rich-presentation.ts:9) | Drift. 016-T03/T04 own the fix. |
| TEXT_SLICE as Markdown | INTELLIGENT-UI-AND-IMAGE-CONTENT.md:314 | Rendered as a plain `<p>` (RichResponseView.tsx:8) | Loses formatting. |
| Declarative components (TABS, DATA_TABLE, METRIC_CARD, CHART, FORM_INPUT, SAFE_CALCULATOR, IMAGE) | INTELLIGENT-UI §3.2 | None | No. 016-T05..T07 own the first two. |
| Interactive Artifact Runtime | INTERACTIVE-ARTIFACT-RUNTIME.md | None | No. 016-T11 keeps it PREVIEW_ONLY. |
| Artifact preview | ARTIFACTS-EVIDENCE.md | Text formats up to 1 MiB (ArtifactWorkbench.tsx:232, :503) | md, csv, tsv, json, xml, yaml only. No HTML, PDF, image or Office preview. |
| Agent-produced file to Library | ARCHITECTURE §10 | No create route or producer | No path exists. 007-T02 and 007-T12. |
| Workbench Files | V1-UI-SCREEN-CONTRACT | WorkbenchPanel.tsx:431-523 | Read-only exact-revision preview. |
| Workbench Terminal, Review | V1-INTEGRATED-TERMINAL-LLD | Honest unavailable notices (WorkbenchPanel.tsx:166-208) | No. 008-T08..T15. |
| Workbench Browser | CHROME-BROWSER-LLD | iframe, `sandbox="allow-scripts allow-same-origin allow-forms allow-popups"` (WorkbenchPanel.tsx:597) | Preview only. See U-02. |
| Workbench Activity | V1-UI-SCREEN-CONTRACT | Type member only (WorkbenchPanel.tsx:11) | No. 008-T02 and 008-T14. |

The WebView CSP (`default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' asset: http://asset.localhost data:`, tauri.conf.json:25) and the `core:default`-only capability set mean agent HTML or JavaScript cannot run today. Keep it that way: the path to richer output is typed blocks rendered by host components, never model markup.

## 4. UI screens

| Screen | File | State | Main defects |
|---|---|---|---|
| Shell and sidebar | App.tsx:2237-2486 | Wired except labels | "Ctrl N" label has no handler (2328). Back/Forward call `window.history.back/forward` (2266-2287) but nothing calls `pushState`, so they do nothing or leave the app. Nav buttons lack `aria-current`, which the focus-restore code queries (676-677). The exported `navigation` array (442-456) is unused and disagrees with the sidebar. |
| Home | App.tsx:2742-3049 | Wired, best state coverage | Disabled Submit reasons are not stated inline. |
| Conversations | conversations/ConversationsPage.tsx | Wired except 3 controls | "More options" (818), "Add context or file" (970) and "Voice Input" (998) have no `onClick`. Icon-only buttons use `title` without `aria-label`. The shell topbar (with the mobile menu) is hidden on this page (App.tsx:2492). |
| Work | App.tsx:3050-3560 | Wired read paths | No Plan/Attempt timeline (execution not served). |
| Library | App.tsx:3752-~4520 | Wired | About 770 lines inline in App.tsx; raw enum text in freshness labels. |
| Needs You | needs-you/*.tsx | Wired | No sidebar count badge (contract section 1 asks for a real count). |
| Coworkers, Automations, Discover | coworkers/, automations/, discover/ | Wired | Honest copy; execution still unavailable by design. |
| Routines | routines/RoutinesPage.tsx | Wired | Reachable only from the Automations template flow (2652, 2654). |
| Goals, Notifications, Ideas | goals/, notifications/, suggestions/ | Wired code, **unreachable** | No nav entry and no `requestPageNavigation` call reaches them. Goals also lacks the offline and no-Workspace gates its siblings have (2606). |
| Fallback | App.tsx:3721 PlaceholderPage | n/a | When an API object is null the page says "Nothing in ... yet", which turns unavailable into empty. |
| Workbench | workbench/WorkbenchPanel.tsx | Partial | Resizer is mouse-only, has no `aria-valuenow`, and persists a stale width from the closure (App.tsx:607-611, 2667-2680). |

| ID | Severity | Finding | Owner task |
|---|---|---|---|
| U-01 | High | Goals, Notifications and Ideas unreachable. | Proposed P-05 (011 shell). |
| U-02 | High (security hardening) | Browser iframe `allow-scripts` plus `allow-same-origin` defeats the iframe sandbox. The CSP currently blocks remote frames in a packaged build, so the exposure is limited, but the attribute pair must not survive any future CSP change. Drop `allow-same-origin` (or the whole embedded preview) now. | Proposed P-06 (008-T02 scope). |
| U-03 | High | Three inert Conversation controls. | 001-T18 (remove inert or wire), 001-T14 (attachments). |
| U-04 | Medium | Back/Forward misleading. | Proposed P-05. |
| U-05 | Medium | Null API falls through to an "empty" placeholder. | Proposed P-05. |
| U-06 | Medium | No theming: `:root{color-scheme: light}` only (styles.css:3); no dark mode, no `data-theme`. | 011-T07. |
| U-07 | Medium | No global shortcuts; `outline: none` in 8 CSS rules without a verified replacement focus style. | 011-T11. |
| U-08 | Medium | Recent Chats shows "No saved chats yet" while loading or after a load error, and has no search. | 014-T11 (search); proposed P-05 for states. |
| U-09 | Low | Duplicated offline panel markup (6 copies), duplicated Workbench toggle and preview reset. | Proposed P-07 (structure). |
| U-10 | Low | No i18n; `toLocaleString()` without a locale setting. | Out of V1 scope; record as a known limit. |

Stale documentation found: V1-UI-SCREEN-CONTRACT.md section 9 and SETTINGS-AND-SHELL-SURFACE-AUDIT.md section 5 still describe a simulated Terminal and synthetic Diffs, which register A-51 removed, and still call the Browser iframe "sandboxed". 008-T18 and 011-T18 own those doc syncs.

## 5. Settings

Mounted: `settings/SettingsView.tsx` with four categories (General, Workspaces, Agents, Runtime). `SettingsPage` (App.tsx:3563-3719) is exported but never rendered, and it holds features the mounted view lacks: instruction-conflict recovery ("Use this version"), revision history, the Runtime enrollment card and the archive checklist.

| Category | Control | Status | Evidence |
|---|---|---|---|
| General | Theme, Layout density, Text scale, Reduce motion | INERT (component `useState` only) | SettingsView.tsx:94-97, :173-235 |
| General | Ctrl + N, Ctrl + W, Ctrl + K | INERT labels, no handler | SettingsView.tsx:247-269; App.tsx keydown handlers cover Escape and Tab only (681, 715) |
| Workspaces | Refresh, Create (name, submit), Instructions (textarea, Save Revision), Archive (two-step) | WIRED | SettingsView.tsx:292-457; App.tsx:1384, :1626, :1674 |
| Workspaces | Current Workspace select | LOCAL-ONLY (localStorage) | App.tsx:825, :1362 |
| Workspaces | Storage policy | Read-only display; `update_workspace_policy` exists but no mounted control | SettingsView.tsx:318-334; App.tsx:1567 |
| Agents | Registry refresh, search, load more; Probe Codex, Probe OpenCode; create, enable, disable binding; set and clear default lead; quota; worker profiles create, revise, duplicate, disable, archive | WIRED | AgentCatalogSettings.tsx:640-1131 |
| Agents | Native controls inspect | WIRED but the route returns 503 | AgentCatalogSettings.tsx:889-893 |
| Runtime | Start Local Runtime, Refresh Status | WIRED | SettingsView.tsx:537-547 |
| Runtime | "Connected (Localhost)", "Daemon is currently disconnected" | Wrong copy (transport is a Unix socket or named pipe; the second shows whenever Operator readiness is false) | SettingsView.tsx:533, :560 |

| ID | Severity | Finding | Owner task |
|---|---|---|---|
| S-01 | High | Accepted B-29 requires each non-served category to render visibly unavailable with its prerequisite; 12 categories are simply absent. | 011-T06. |
| S-02 | High | 7 inert General controls stay enabled. | 011-T06 (remove until wired), 011-T07 (persist and apply), 011-T11 (shortcut registry). |
| S-03 | Medium | Copy overclaims: "Workspaces & Roots" and storage-policy management (SettingsView.tsx:280-282); "Claude Code, Gemini" detection (:468). | 011-T06. |
| S-04 | Medium | About 16 props are passed but never read, including `runtimeError` and lifecycle loading/error, so Settings shows no Runtime error. | 011-T05, 011-T06. |
| S-05 | Medium | Unmounted `SettingsPage` holds features users cannot reach. | Proposed P-08: merge into the mounted view, then delete. |
| S-06 | Medium | Settings live in four places with no stated precedence: Workspace default lead (Settings) vs Coworker default lead (CoworkerSettings.tsx:61, :537, :588); notification policy in NotificationsPage, Coworker (CoworkerSettings.tsx:79) and Automation (AutomationEditor.tsx:69, :114); two time-zone sources. | 011-T10 (notifications entry); proposed P-08 for a precedence note on each surface. |
| S-07 | Low | Ledger LOCAL_PREF wording ("local selection/draft state only") conflicts with screen contract section 11 ("actually applied and stored locally"). | 011-T01 ledger promotion. |

Recommended V1 Settings shape (within accepted B-29/B-33): General (wired appearance or nothing), Notifications (link to the served policy page with precedence text), Workspaces (add the merged `SettingsPage` features and a link to Library root operations), Agents (fixed copy; model and effort shown unavailable with the 503 reason), Coworkers and Subagents (worker profiles plus a link to the Coworker editor), Runtime (fixed copy, `runtimeError` rendered). Every other category appears as a non-interactive row naming the missing owner operation and package (for example "Storage & Backup: needs Feature 017").

## 6. Architecture

| ID | Severity | Finding | Evidence |
|---|---|---|---|
| X-01 | High | No adapter event-normalization layer. Provider events go straight from JSON into supervisor heuristics. ARCHITECTURE.md and AGENTS.md require provider events to be normalized and correlated before use. | supervisor.rs:533-557 |
| X-02 | Medium | Very large files: storage-sqlite/src/lib.rs 23,985 lines; operator.rs 9,525 (router, IPC accept/dispatch, handlers); src-tauri lib.rs 6,132 (45 commands plus validators); generated operator-api.ts 13,833; App.tsx 4,523. | `wc -l` |
| X-03 | Medium | Layering is inverted in naming: `domain-workspace`, `domain-task` and `domain-rag` depend on `storage-core`, which itself owns many domain records. storage-core is the de facto domain model. Documented as accepted placement for most records (register A-41). | domain-workspace/src/lib.rs:2; storage-core/src/lib.rs:6-203 |
| X-04 | Medium | Request validation duplicated in the Tauri bridge and the daemon. Defense in depth, but two sources of truth for limits. | src-tauri/src/lib.rs:868, :1400, :2892, :2964, :3312, :3609, :3622 |
| X-05 | Low | Typed errors (`code`, `retryable`, `correlation_id`) are flattened into one string at the Tauri boundary, so the UI parses strings. | src-tauri/src/lib.rs:4975-4990 |
| X-06 | Low | Each Tauri command uses `spawn_blocking` and then `block_on` on a second private tokio runtime. Safe, but holds a blocking thread per request. | src-tauri/src/lib.rs:4357-4420 |
| X-07 | OK | IPC is OS-authenticated (SO_PEERCRED uid check before any frame; Windows named-pipe DACL); no production SQL in daemon handlers (rusqlite only in `mod tests`); src-tauri depends only on `operator-ipc` and `windows-local-security`; streams are projections with per-frame ack. | operator.rs:381-490, :1700-1750; conversation_stream_bridge.rs:60-114 |

Rust vs TypeScript placement, consistent with the per-package optimization notes added on 2026-11-09:
- **Rust:** provider event normalization (X-01), the single rich-presentation validator (016-T03), Markdown-to-TABLE binding after commit (016-T08), diffs, PTY, and typed error codes.
- **TypeScript:** pure reducers and renderers only. The TS rich validator becomes a parity check generated from docs/schemas, not a second authority.
- **Split order for large files:** operator.rs into `ipc_server.rs` plus a router module; src-tauri lib.rs into the existing `*_bridge.rs` pattern; App.tsx into `shell/` plus one file per page (LibraryPage, HomePage, WorkPage first).

## 7. What "rendering properly" needs, end to end

1. **Normalize (Rust, daemon).** An `OpenCodeEventNormalizer` beside opencode_server.rs maps v2.0.32 events to an internal enum: `TextDelta`, `TextEnded`, `ReasoningStarted/Ended` (content never shown or stored unless the harness marks it user-visible), `ToolCalled/Progress/Succeeded/Failed {name, safe summary}`, `PermissionAsked`, `FileChanged {path}`, `TodoUpdated {items}`, `StepStarted/Ended`, `UsageRecorded`, `ExecutionSucceeded/Failed`, `Idle`. Correlate on the session id in the envelope's `data` or `location`, never on a top-level guess. Capture a real v2.0.32 turn as a redacted fixture first.
2. **Stream (Rust).** Coalesce `TextDelta` into `turn.delta` frames every 50-100 ms using the frame shape the UI reducer already accepts. Add a transient `turn.activity` frame for tool, file, permission and todo events.
3. **Commit (Rust, storage).** The committed AGENT message stays semantic text plus RESOURCE refs. Tool and file activity are stored as REPORTED activity observations linked to the turn, never as LiteCowork Effects (AGENTS.md: direct native actions are not Effects). Usage goes to Feature 018.
4. **Render (TypeScript).** Per-block Markdown degradation with GFM tables, rules and nested lists; code blocks with language label, copy and lazy highlighting that returns React nodes; inline FAILED-turn card with the safe reason; activity cards that collapse by default; stick-to-bottom autoscroll; list virtualization for long chats.

## 8. Making it more cowork-like

Reference behavior, from public descriptions of Claude Cowork: it works on folders the user grants, runs in the background, asks when something is unclear, and delivers finished files as artifacts ([Vellum breakdown](https://www.vellum.ai/blog/official-claude-cowork-breakdown); [setup guide](https://macaron.im/blog/how-to-enable-claude-cowork)). Later 2026 additions include live artifacts that refresh from connected apps, routines, and chat and Cowork merging into one surface ([Substack summary](https://open.substack.com/pub/claudedesktop/p/the-one-cowork-feature-that-replaces); [October 2026 updates](https://aimaker.substack.com/p/claude-code-updates-october-2026)). These are third-party summaries, treated as research and not as verified product facts. Content was rephrased for compliance with licensing restrictions.

LiteCowork already owns the right model. The gap is that the working state is invisible. In priority order:

| # | Change | Why it matters | Invariant guard | Where it lands |
|---|---|---|---|---|
| C-1 | Live plan panel: render the native harness todo list (`TodoUpdated`) as a checklist beside the chat. | The user sees what the agent will do and how far it got. | Labelled "reported by the agent"; never a LiteCowork Plan or Task. | Proposed P-01/P-02 (001 UI) |
| C-2 | Activity cards for tool calls and file changes inside the turn, with path and +/- counts. | "What is it doing" is the core cowork question. | REPORTED observations, not Effects; no fabricated success. | P-01/P-02 |
| C-3 | Permission prompts surfaced inline and in Needs You when the harness asks (`PermissionAsked`), answered back to the harness. | Matches "asks when unclear, you keep the final say". | Native permission, not a LiteCowork grant; copy must say so. | Proposed P-09 (001 with 002) |
| C-4 | Outputs tray: files the agent changed in a granted root, listed per turn with Open, Reveal and a real diff; one click imports a chosen file as an Artifact. | Deliverables, not just text. | Import is the user's explicit action through 007 create (imported source class). | 007-T12, 008-T15, P-02 |
| C-5 | Folder-first start: Home and New Chat offer "Work in a folder", which adds or selects a WorkspaceRoot and attaches it as context. | Cowork starts from your files. | Root grant stays the Workspace boundary (003). | 003-T08, 003-T11 |
| C-6 | Background completion: a turn or Task that finishes while the window is unfocused raises a local notification and a sidebar badge. | Hand off and come back. | LOCAL_POLL delivery only; no laptop-off claim (A-16). | 010-T06, P-05 badge |
| C-7 | One-click "Make this a Task" from any agent reply, prefilled from the turn. | Chat-first, explicit Task creation. | Creation stays an explicit owner action (A-06). | 005-T12 |
| C-8 | Rich answers: tables, charts and a calculator rendered from typed blocks with a text fallback. | Answers you can read and use. | No model HTML/JS; 016 validator is the boundary. | 016-T03..T09 |
| C-9 | Split view: two Conversations or a Conversation plus a Library item side by side. | Parallel work, as later Claude desktop builds do. | Pure UI layout; no new authority. | Proposed P-10 (011 shell, post-V1 if needed) |
| C-10 | Live artifacts that refresh from a Connection. | Strong rival feature. | Needs 006 Connections and 016 runtime; defer past V1. | Record only |

## 9. Finding to task map and proposed task additions (accepted 2026-11-09, register A-55)

Existing tasks already cover: R-02 (001-T04), R-03 (001-T13, 001-T16), R-07 typed failure (001-T04), R-09 (001-T14), U-03 (001-T14, 001-T18), U-06 (011-T07), U-07 (011-T11), U-08 search (014-T11), S-01..S-04 (011-T05, 011-T06, 011-T07, 011-T11), rich drift (016-T03, 016-T04), Activity tab (008-T02, 008-T14), Artifact create (007-T02, 007-T12).

Proposed additions for owner review. Packages 001, 008 and 011 are ACCEPTED/LOCKED, so these need an owner-approved TASKS revision; nothing here changes the locked files.

| ID | Package | Proposed task | Acceptance sketch |
|---|---|---|---|
| P-00 | 001 (before 001-T04) | Capture a redacted OpenCode v2.0.32 turn (event stream for a text answer, a tool call, a file edit, a permission ask, an error) as a committed fixture and record the exact envelope in RESEARCH.md. | Fixture replays through the parser; R-01 confirmed or refuted with evidence. |
| P-01 | 001 | `OpenCodeEventNormalizer` (Rust) with the enum in Section 7, correlation on the envelope session id, reasoning kept separate, idle or heartbeat timeout replacing the 60 s cap. | Fixture tests per event kind; unknown events counted and dropped; no reasoning text in the committed message. |
| P-02 | 001 | Transient `turn.activity` frames and REPORTED activity records; UI activity cards, plan checklist and inline FAILED card. | Activity visible during a turn and after restart from records; copy says "reported by the agent". |
| P-03 | 016 (folded into T18; package IN_REVIEW) | Per-block Markdown with GFM tables, rules, nested lists, code language label, copy and lazy highlighting; TEXT_SLICE uses the same renderer. | Renderer and authored adversarial checks are implemented. Real native-agent answer corpus remains open; no HTML strings are generated. |
| P-04 | 001 | Stick-to-bottom autoscroll and "new messages" indicator. | Scroll stays when the user reads history; follows when at bottom. |
| P-05 | 011 | Shell reachability and truthfulness: nav entries for Goals, Notifications and Ideas (or remove them from V1 with owner agreement); remove or implement Back/Forward; unavailable vs empty fallback; Recent Chats loading and error states; Needs You count. | A test fails if a mounted page has no navigation path or a null API renders an empty state. |
| P-06 | 008 | Remove `allow-same-origin` from the Browser preview iframe (or remove the preview) and add a test that rejects the pair. | Test in workbench-truthfulness fails on `allow-scripts` with `allow-same-origin`. |
| P-07 | 011 | Split App.tsx into `shell/` and one module per page; one shared "Operator unavailable" panel. | No behavior change; existing UI tests pass. |
| P-08 | 011 | Merge the unmounted `SettingsPage` features into SettingsView, delete `SettingsPage` and the unused `navigation` array, and add precedence notes where a setting has a Coworker or Automation override. | One Settings authority; conflict recovery and enrollment reachable. |
| P-09 | 001 with 002 | Native permission prompts surfaced inline and in Needs You, answered through the harness API. | A denied prompt reaches the harness; no LiteCowork grant is created. |
| P-10 | 011 | Split view (optional, owner decides V1 or later). | Layout only. |

## 10. Method and limits

Four parallel read-only source traces (agent output pipeline, UI screens, Settings, architecture and Intelligent UI) plus direct verification of the highest-severity lines: supervisor.rs:526-575, WorkbenchPanel.tsx:597, tauri.conf.json:25, StructuredTextPreview.tsx:92-155, ConversationsPage.tsx:818, :970, :998, and the absence of navigation calls to Goals, Notifications and Ideas and of any `pushState` in App.tsx. The OpenCode event shape comes from the published v2 API reference and a third-party schema digest of an earlier commit ([opencode-plugins-manual event schemas](https://github.com/joshuadavidthomas/opencode-plugins-manual/blob/main/docs/appendix/event-schemas.md)); it has not been observed against a running v2.0.32 server in this repo. No contrast measurement, packaged-app run or test run was done.
