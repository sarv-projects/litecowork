# ZCode UI/UX Inspiration Audit for LiteCowork

**Review date:** 2026-10-31  
**Source repository:** [zai-org/ZCode](https://github.com/zai-org/ZCode)  
**Reviewed ref:** `main` at `a60ba6a7e5a06742270ca605f5d2cc2766afa7e1`  
**Purpose:** extract reusable product and interaction patterns from the supplied ZCode screenshots and public implementation/specifications. This is a subordinate inspiration/review document, not a new security authority, runtime architecture, or implementation approval.

## 1. Scope and confidence

The repository tree was fetched from GitHub at the reviewed ref. It contained **18,340 tree entries**, was not truncated, and represented approximately **17,203 tracked blobs**. ZCode is a large TypeScript/TSX monorepo with apps, shared packages, UI, CLI/runtime, documentation and vendor/upstream material.

The review visually inspected all 11 user-supplied screenshots and read relevant ZCode product/implementation references and source files in full where feasible. Principal sources are listed in §7. Large source files were fetched directly (including the 1,211-line Computer Use SDK and 1,104-line context-usage component), but the connector truncated some multi-file response output. This is therefore a targeted source-grounded review, **not a claim that every line of every one of the roughly 17,203 repository blobs was read**. Vendor snapshots, binary assets, lockfiles and unrelated domains were not line-by-line reviewed.

The distinctions below matter:
- **Observed in screenshot:** visible UI, copy, status or affordance.
- **Specified by ZCode:** behavior defined in its own design/owner documentation.
- **Source-verified:** behavior confirmed in fetched implementation source.
- **LiteCowork proposal:** a recommendation for consideration, not a current feature or approved requirement.

## 2. Screenshot inventory — the small details are the product

### 2.1 Remote devices settings

The Remote devices page shows a searchable device list and a distinct **New connection** action. A device row combines device identity and OS/distribution/user information, last-online age, a visibly actionable state (`Update required`), a contextual primary action (`Update remote device`), an overflow menu and disclosure affordance.

Below the list, “Unfinished connections” is a separate disclosure section rather than being mixed into the normal device list. Preferences are independent, labeled toggles:
- “Hide device names in project rows”, with helper text explaining where names disappear and that hover reveals them.
- “Reconnect automatically at startup”, with helper text spelling out the seven-day eligibility window and that the new value takes effect only at the next startup.

**Borrow:** make the state, age, device identity, available repair action and preference consequence readable in the row/page itself. A toggle should state what it changes and when the change becomes effective. Keep incomplete/pending connections separate from established devices.

**Do not copy blindly:** ZCode's remote-device surface is more developed than LiteCowork V1's local-first desktop release boundary. Remote-device management must remain hidden/unavailable until a LiteCowork remote Environment/Runtime route is in scope and qualified.

### 2.2 Browser Use settings

The Browser Use page has three clear groups:

1. **Enable built-in browser control** — capability enablement and explanatory copy say that new sessions can use the built-in browser.
2. **Security** — “Ignore certificate errors” is separately labeled, off in the screenshot, and says that it disables HTTPS certificate verification, affects only the built-in browser and requires a restart.
3. **Browser data** — “Clear built-in browser cache” says it removes HTTP cache, Cache Storage and service workers **while retaining cookies and local site data**. “Clear all browser data” says cookies, site data and cache will be deleted and that this cannot be undone. It uses a destructive red action.

**Borrow:** separate capability activation, security controls and data lifecycle. Spell out exactly what is removed and what is retained. Never make safer cache cleanup visually indistinguishable from a complete profile/site-data wipe.

**LiteCowork security conflict:** `CHROME-BROWSER-LLD.md` requires TLS verification and explicit consent for existing Chrome. A general-purpose “ignore certificate errors” toggle would weaken that invariant. Do not port this as a normal LiteCowork feature. If a narrowly scoped developer-only diagnostic override is ever considered, it needs a separate owner decision, a highly visible insecure state, exact scope, expiry and restart/revert behavior; it must never silently flow into agent permissions or production use.

### 2.3 Computer Use settings

The Computer Use page shows two separate controls:
- **Enable Computer Use** — enables the capability, its MCP server and skills.
- **Show Computer Use button in the composer** — controls the visible Composer affordance only; turning the button off does not imply the capability itself has been disabled.

**Borrow:** capability availability and surface visibility are different settings. Hiding a Composer shortcut is not the same as revoking a capability or its authorization. LiteCowork should avoid a single ambiguous switch that silently means both “disable runtime” and “hide button”.

### 2.4 Main workspace, add menu and right pane

The screenshots show a left task/navigation rail, one central conversation/composer, and a separate right-side pane with Files, Browser, Terminal and Review as available workspace surfaces. The add menu is divided into Attachments, Workflow and Plugins. Plugin rows have an icon, title and one-line explanation. Bottom affordances expose “Add context”, “Choose capabilities” and “Choose skills”, plus a search hint that can find plugins, files and chats.

**Borrow:** use progressive disclosure rather than exposing every tool/capability in the Composer by default. Group things by the user's intent (attach material, select an execution capability, choose a procedure/skill, invoke a workflow). Each item needs a one-line explanation of the outcome, not just an internal package name. Keep optional right-side work surfaces distinct from the conversation transcript.

### 2.5 Context-window popover

The screenshot exposes a compact context diagnostic without making it part of the primary conversation: used/maximum tokens and percentage; a segmented bar; a source legend with percentages; cache hit rate; and separate provider quota/balance information. The sample legend showed System tools 81.2%, MCP tools 8.9%, Skills 5.2%, System prompt 2.6%, Messages 1.4%, and Meta context 0.7%, plus 91.6% average cache hit rate.

**Important source detail:** ZCode's `contextUsageBreakdown` is estimated by **character count**, while total used/maximum comes from provider token usage. The segmented bar divides the already-existing used portion of the header bar; it does not add a second bar or pretend the character percentages are token percentages. The breakdown is optional. If absent, the UI falls back to the ordinary solid used bar. Low cache hit rates are hidden in the compact production popover below a 78% aggregate threshold; missing provider data is not guessed.

**Borrow:** one compact breakdown in the context popover, source names in plain language, percentages instead of raw internal payload, and no fake fallback values. Explain the measurement basis. Keep raw prompt/tool-schema contents out of the user-facing legend.

### 2.6 Computer Use task/progress and completion

The screenshots show a small interaction sequence: the user asks for one-time access/demo; the assistant states the intended method in natural language (open a browser, navigate, click); it identifies the selected skill/capability (`browser-use:control-browser`); the task exposes Running/Working; if the command needs approval, the task changes to “Permission required / Awaiting approval” and shows the exact command; on completion it reports “Computer accessed successfully” with a compact facts table (user, computer name, OS/build), followed by what it actually demonstrated.

**Borrow:** before a potentially surprising action, say which capability and surface will be used and why. Distinguish queued/running/waiting-for-approval/completed states. On success, show a factual receipt of observed device/OS/scope rather than a generic green check or an overbroad claim such as “full computer access” when only one capability was granted.

### 2.7 Two-stage consent: global access and per-action approval

The screenshots show two separate approval levels.

**Global “Enable full access?” acknowledgement** discloses three capability groups: files and folders (read/create/change/delete accessible files); terminal commands (run commands, install software, change system settings); and network/apps (access the network, send data, use connected apps). It warns about data loss/disclosure, requires an explicit acknowledgement checkbox, and offers Cancel/Confirm.

**Per-command permission card** then offers: (1) Allow only this time; (2) Always allow this command, with copy saying matching commands in the project will not prompt again; (3) Full access; (4) Deny; and (5) Tell the model what to do instead. It displays the actual command and an explicit confirmation step. The “always” scope is explained rather than left to inference from the button label.

**Borrow:** make standing authorization, one-time approval, broad task-mode change and denial four separate concepts. Display the exact requested operation, target and scope. Offer a safe “tell the agent another way” route that is not itself an approval. Do not make the user infer whether “always” is per command, Workspace, project, device or capability family.

## 3. What ZCode's specifications and implementation add

### 3.1 Computer Use is not Browser Use

ZCode separates Browser Use (web-page control) from Computer Use (native applications/OS UI). The official Browser Use plugin gates the browser skill/bridge, while the host retains the node_repl identity and browser/CDP infrastructure. Disabling the plugin removes the Browser bridge/skill for new sessions; it does not transfer host tool identity to an arbitrary plugin. Computer Use has its own SDK, skill and brokered host interface.

The Computer Use skill prefers purpose-built connectors/APIs/CLIs, uses Browser Use for web pages and Computer Use for native apps, prefers accessibility semantics before keyboard and raster coordinates, and requires observe → act → re-observe. It is main-agent-only and never delegates native UI control to a subagent. Access refusal or Stop/kill-switch means stop; another automation technology must not be used to route around denial.

**Source-verified SDK details:** the 1,211-line client wraps a shared host broker. Each JS worker is fresh per call, so local bindings do not survive; the host owns state/raster continuity and the SDK rebinds cheaply. Actions expose `actionSent` and retry classification. If an action may already have been sent, the client requires observation before retry; non-idempotent clicks/typing are not blindly replayed. Stale element/window/raster and controller-busy failures are typed and fail closed. Helper-not-ready is retried only when its receipt explicitly says retryable, with bounded backoff.

This is not just a convenient API design. It prevents a user-facing retry button or agent loop from duplicating a click, form submission or other side effect.

### 3.2 Permission is a protocol, not merely a modal

`docs/permission-project-approval.md` defines stable `optionId`, display semantics, copy and the exact protocol response for each option. The renderer does not infer an allow decision from a button's numeric position. `allow_project` returns `permissionUpdates`; it is not silently reduced to allow-once.

Additional behavior in the owner contract:
- Official CUA project-level grants are capability-group scoped only when official plugin authority is verified; same-named third-party MCP tools cannot spoof the group.
- Deny-with-feedback is an input on Deny, not a fourth decision or a real chat message.
- The separate `fullAccessOption` is backward-compatible and fail-closed.
- Full access uses the interaction broker/registry, Runtime owner and atomic store transaction; the current tool is not released until authoritative V4 projection is confirmed. Failure leaves approval pending/retryable.
- The permission card is reused across desktop continuous and web/mobile replayable delivery.
- Child-origin approvals and interactions that cannot safely implement full access do not advertise it.

The separate “full access” mode in the screenshot is therefore not equivalent to a project grant. It is a distinct, task-scoped operation with explicit state/queue effects in the ZCode implementation.

### 3.3 Remote-device lifecycle is a shared projection

ZCode's current remote UX spec defines one five-state model shared by device rows, sidebar/project rows, connection wizard, headers and task-row device tags: Connected; Connecting; Disconnected; Needs attention (installation, mandatory update, identity confirmation or missing credentials); and Connection failed.

One projection owns status copy, semantic color, icon, primary action, repair path, recovery progress and connection description. UI surfaces must not each invent their own state mapping. “Needs attention” has priority over “connection failed”; explicit user disconnect is always “disconnected”; optional updates do not falsely change connection state; non-online states need text, not color alone.

The connection flow is a searchable unified device list; offline recent-directory entries remain actionable and preserve intent; discovery from multiple sources is nonblocking; forms use real semantics, field-level validation and predictable focus; collapse preserves the same flow; cancel closes the request epoch so late credentials/results cannot resurrect an old flow; success continues to the requested directory. Device stop/restart/reinstall actions inspect running tasks first and explain interruption before confirmation, with Cancel focused by default. Logs/toasts preserve the operation outcome.

This is a reusable pattern for LiteCowork's eventual Environment maintenance, but not a reason to pull remote-device management into local-only V1.

### 3.4 Context display keeps units honest

ZCode's context data flow is main-session scoped. It excludes sidecar requests (compaction, title generation, subagent/workflow children) from main-session cache-hit aggregation. The hit rate is token-weighted using provider usage fields; the UI source breakdown is separately character-weighted. It is carried through the same snapshot/stream path for desktop and replayable remote UI. A newer snapshot that temporarily lacks a breakdown retains the last valid breakdown for the same context window until a newer breakdown arrives.

The implementation filters invalid/zero character entries, groups by source and sorts by descending share, uses a low-saturation blue scale instead of the general chart rainbow palette, uses localized compact counts and percentages, does not fabricate cache metrics when the provider omits them, and keeps the breakdown optional for older Agent versions.

### 3.5 Data-management affordances are deliberately scoped

The browser page distinguishes cache cleanup from full site-data deletion. ZCode's storage-management contract uses a catalog describing category, root, cleanability and protected paths, and distinguishes live DB/session data from diagnostic traces, backups, caches and tool artifacts. It explicitly avoids treating all storage under one root as equally disposable.

The transferable point is the classification and the copy: explain what a cleanup deletes, what it preserves, whether it is reversible, and whether it can affect a running task/session. Never combine cache, credentials, cookies, local site data and durable task history under one vague “Clear data” action.

## 4. Recommendations for LiteCowork

These recommendations are **inspiration**, not accepted changes. They must be checked against `ARCHITECTURE.md`, `PRODUCT.md`, `V1-PRODUCT-RELEASE-CONTRACT.md`, domain owners and machine contracts before becoming requirements.

### P0 — permission and access surfaces

1. **Separate capability availability from Composer visibility.** “Computer Use enabled” and “Show Computer Use in Composer” should be independent if both controls are exposed. Hiding the button must not revoke an existing capability grant; disabling the provider must not be represented as merely hiding UI.
2. **Use an explicit scope-first permission card.** Summarize verb, target, resource, Workspace/Task/Attempt scope, expected side effects and whether the action is destructive. Show the exact shell command or native action details when appropriate. Use stable option IDs and owner-defined responses; never derive permission from UI ordering.
3. **Keep one-time, standing grant and broad execution mode distinct.** Labels must name the actual scope: one invocation, exact repeated action within an explicitly named scope, or a broad Task execution mode. “Always allow” must never leave the scope implicit.
4. **Treat broad access as a separate consent operation.** Before enabling it, disclose the actual file roots, process/terminal power and network/app exposure. Require a separate acknowledgement and confirmation. Keep the permission decision and broad-mode transition as distinct auditable records if owners define them that way. Do not port ZCode's `yolo` semantics or queue update behavior into LiteCowork without a Trust/Runtime owner decision.
5. **Keep “Tell the agent what to do instead” out of the allow list.** It is a safe instruction/refinement path, not a permission option. Deny-with-feedback may be considered only if Trust and API owners define it as data on the existing denial response; never turn it into a normal user message or grant.
6. **Make keyboard and accessibility behavior first-class.** Default focus on the safest option; support predictable Tab/Shift+Tab, arrow and numeric selection; visible focus; separate feedback input from listbox semantics; Enter confirms only the focused option; Escape cancels/returns focus without granting.
7. **Fail closed on unsupported clients and stale approvals.** Unknown options are denial, not allow. Approval binds to exact action/target/scope digest. If action or target changes, request approval again. If dispatch is ambiguous, show “checking/reconciling”; do not offer blind retry for a non-idempotent operation.

### P0 — Computer Use interaction

1. **Keep Browser and Computer capabilities distinct.** Browser Use operates on web content; Computer Use operates native applications and OS surfaces. Show the active surface in the task transcript, and do not imply browser permissions grant desktop control.
2. **Prefer APIs, then structured browser/accessibility, then native GUI.** Use Computer Use only when the action genuinely requires OS UI. The chosen method should be visible in a compact activity row, not verbose explanation for every click.
3. **Use progressive access readiness.** Show provider/adapter availability, OS support, helper version/health and required OS permissions (for example, Accessibility and Screen Recording) before launching a task. Ask only for minimum required capabilities. Explain denial and repair without routing around it.
4. **Use semantic controls first.** Native app role/name/value/action, then keyboard if no semantic action exists, then a current screenshot coordinate. Always re-observe after state-changing action. A stale screenshot or changed window must invalidate old coordinates/indices.
5. **Make retries evidence-based.** Show “Action may have been sent — checking result” when dispatch is ambiguous. Only retry after owner reconciles not-sent or safe same-key replay. For controller-busy, show which session owns input and provide a handoff path. Never double-click/replay a destructive action because a spinner lasted too long.
6. **Show a truthful access receipt.** On first successful access, show observed computer name, OS/build, helper/provider version and actual granted scopes. A successful one-time click must not be summarized as broad “full computer access”.
7. **Make Stop/revoke immediate and visible.** Show current input owner (agent/user/none), control lease/epoch changes, and that new actions are fenced after Stop/revoke. Browser or Computer UI pane close, session close and stopping a Task are separate actions with explicit impact.

### P1 — Browser and data management

1. Provide separate sections for browser capability, security and data management.
2. Keep TLS/certificate validation enabled. A broad “ignore certificate errors” toggle conflicts with LiteCowork's TLS invariant; do not introduce one into ordinary settings.
3. Offer cache cleanup separately from full profile/site-data deletion. Explain exactly which categories are cleared and preserved. Full deletion is destructive, scoped to one profile/Workspace, requires impact confirmation and never silently removes download records or durable Conversation/Task history.
4. Show profile identity, Workspace assignment, current user/agent control owner, open-tab count and last-used age only from authoritative data. Never derive “healthy” from cached or stale UI state.
5. Surface storage categories from a single owner catalog with `cleanable` and protected-path semantics; never let UI heuristics decide whether session DBs, backups, tool artifacts or credentials are disposable.

### P1 — Remote/Environment management (only when in product scope)

1. A device/Environment row should show a stable display name, OS/platform, connection method, last observed online time, current lifecycle state and one primary repair/action. Use an overflow menu for less common actions.
2. Give each lifecycle state explicit copy and action: Connected, Connecting, Offline/Disconnected, Needs attention, Failed/Unknown. Do not let each screen map state independently.
3. Show required update vs optional update clearly. A mandatory update may offer Update as the primary action; an optional update is informational and must not look like disconnection.
4. A startup reconnect preference must specify its eligibility window and when the change takes effect. Show a “pending next startup” hint if appropriate; distinguish automatic retry from explicit user connection.
5. Keep pending/unfinished connections separate from established devices. Preserve collapsed connection progress and provide resume/cancel; cancellation must fence late results. Show recovery attempts/phase and actionable failure reason.
6. Before stopping/restarting a remote service, show actual affected running Tasks and the consequence (“will interrupt; will not automatically resume” if true). Default focus to Cancel. Record operation outcome and reconcile ambiguous restart state.
7. These affordances are future-scope only while LiteCowork V1 remains local desktop and remote Runtime/device control remains outside the release boundary.

### P1 — Context and usage transparency

1. Keep context usage discoverable in a compact Composer context popover rather than permanently consuming chat space.
2. Show used/maximum/percentage from provider token usage. If a source breakdown is added, label its measurement unit exactly (for example, character-estimated share) and never present it as token allocation.
3. Use one progress bar with segmented used portion, compact legend, muted palette and text labels. No second duplicate meter and no rainbow chart palette.
4. Keep the breakdown optional, main-session scoped and redacted. Do not reveal raw system prompt, tool descriptions, MCP schema contents or secret-bearing context.
5. Preserve the last valid breakdown only for the same context-window identity while a fresher snapshot is incomplete. Clear it when context/turn identity changes. Missing provider values stay unknown.
6. Cache hit rate is a separate provider metric, not inferable from context-source characters. Only show it when valid, clearly aggregated and useful; do not invent a fallback.

### P2 — Tiny interaction polish

- Use one-line helper copy that answers “what changes?”, “what stays?”, and “when does it take effect?”.
- Use specific verbs on buttons: Update device, Clear cache, Clear all browser data, Reconnect at startup.
- Keep destructive actions visually distinct but not visually adjacent to safe actions without a clear divider and copy.
- Make status readable without color alone; include an age (“last online 38m ago”) only when its timestamp is real.
- Keep list search, unfinished/pending disclosures and action menus; avoid a permanent wall of capability toggles.
- For progress, show phase and current step as well as a percentage only when the percentage has a defined denominator. Do not display a fake percentage for an unknown stage.
- On success, present a compact factual receipt; on error, show repair action and a copyable diagnostic redacted by policy.
- Treat “show in composer” as a presentation preference; capability availability and authorization remain in their own owner layers.

## 5. Mapping to LiteCowork's existing canonical contracts

| ZCode inspiration | Existing LiteCowork owner/contract | What to do |
|---|---|---|
| Native Computer Use consent and operation scope | `TRUST.md`, `ENVIRONMENTS.md`, `PRODUCT.md` | Preserve Core Trust authority; improve presentation only after Trust option/response contract is settled. |
| Browser session and takeover | `CHROME-BROWSER-LLD.md`, `ENVIRONMENTS.md`, `API.md`, `EVENTS.md` | Retain session, lease, epoch, explicit existing-Chrome consent, observation and return-to-agent rules. |
| Browser vs Computer split | `CHROME-BROWSER-LLD.md`, `CAPABILITY-FABRIC.md` | Expose independent capability readiness and truthful surface identity. |
| Composer/context window | `NATIVE-COMPOSER-LLD.md`, `PRODUCT.md`, `V1-UI-SCREEN-CONTRACT.md` | Keep a compact context popover; do not add another usage authority. |
| Environment/device lifecycle | `ENVIRONMENTS.md`, `PRODUCT.md`, `V1-PRODUCT-RELEASE-CONTRACT.md` | Use one projection owner if/when remote device management enters a released feature. |
| Cache/profile cleanup | `CHROME-BROWSER-LLD.md`, `STORAGE.md`, `TRUST.md` | Reuse owner catalog/retention and exact-scope confirmation. |
| Permission and broad access | `TRUST.md`, `API.md`, `EVENTS.md`, schemas | No parallel permission protocol; use stable IDs and exact owner responses. |
| Real user acceptance | `ACCEPTANCE-AND-E2E-PLAN.md` | Test exact shipped UI/daemon/provider/OS and positive/negative paths. |

The existing V1 release boundary remains decisive. Remote Runtime/device rows, cross-device reconnect and mobile replay are not pulled into V1 by these screenshots. Browser/Computer UI is a target until providers are qualified; `CHROME-BROWSER-LLD.md` remains the browser owner. Trust remains the only authority for approvals, grants and revocation.

## 6. Decisions not to copy

- Do not copy ZCode's `yolo` mode, queue synchronization transaction, official CUA plugin capability-group grant or permission-rule persistence as LiteCowork semantics. Their meaning belongs to ZCode's Agent/Runtime/Trust owners.
- Do not add an “ignore certificate errors” ordinary toggle; LiteCowork's TLS invariant takes precedence.
- Do not claim access merely because a helper is installed, a button is visible, or an MCP server is registered.
- Do not use shell/CLI as a fallback after Computer Use access denial.
- Do not treat a screenshot/coordinate action as successful without a fresh observation or owner receipt.
- Do not expose raw prompts, tool schemas, cookies, credentials, browser profile paths or full logs in UI/evidence.
- Do not infer remote device health from last known state or UI caches.
- Do not put task history, browser cookies, profile data, caches and diagnostics under one destructive “clear all” action without exact scope and impact.

## 7. Reviewed upstream sources

All links below point to the reviewed ZCode tree at commit `a60ba6a7e5a06742270ca605f5d2cc2766afa7e1`.

- [Computer Use skill](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/apps/zcode-cli/packages/zcode-cua-plugin/skills/computer-use/SKILL.md)
- [Computer Use reference/API](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/apps/zcode-cli/packages/zcode-cua-plugin/docs/computer-use.md)
- [Computer Use SDK](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/apps/zcode-cli/packages/zcode-cua-plugin/scripts/computer-use-client.mjs)
- [Permission and project approval contract](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/docs/permission-project-approval.md)
- [Full-access permission implementation plan and evidence](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/docs/working-memory/provider-refactor/steps/todo-158-permission-dialog-full-access-and-queued-mode.md)
- [Browser Use runtime/plugin boundary](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/docs/browser-use/browser-use-plugin-runtime-boundary.md)
- [Remote-device connection UX](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/docs/architecture/remote-device-connection-ux-spec.md)
- [Context usage UI contract](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/docs/ui/chat-input-context.md)
- [Context cache-hit semantics](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/docs/context-cache-hit-rate.md)
- [Context source breakdown UI spec](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/docs/superpowers/specs/2026-06-11-context-usage-breakdown-bar.md)
- [Context usage component](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/packages/ui/src/chat-input-toolbar/contextUsage.tsx)
- [Source server/device management panel](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/packages/ui/src/settings/SourceServerManagementPanel.tsx)
- [Remote connection flow](https://github.com/zai-org/ZCode/blob/a60ba6a7e5a06742270ca605f5d2cc2766afa7e1/packages/ui/src/remote-connection-flow/index.ts)
- [LiteCowork Chrome Browser LLD](CHROME-BROWSER-LLD.md)
- [LiteCowork Environment LLD](ENVIRONMENTS.md)
- [LiteCowork Trust contract](TRUST.md)
- [LiteCowork acceptance/E2E plan](ACCEPTANCE-AND-E2E-PLAN.md)

## 8. Deep Source Inspection & Architectural Learnings (`repo_take/zcode`)

Direct examination of the local `repo_take/zcode` checkout confirms key architectural and backend logic patterns relevant to LiteCowork's desktop release:

1. **Settings Organization & Category Triad (`packages/ui/src/settings/settingsPageConfig.ts`):**
   - ZCode classifies settings into three strict groups: `basics` (General, Appearance, Model Provider, Browser, Computer Use, Shortcuts, File Search), `agentCapabilities` (Personalization/AGENTS.md, Memory, Subagents, Plugins, MCP, Skills, Commands, Automations, Hooks), and `dataAndStats` (Data Sync, Usage, Archive).
   - Each setting section has a distinct identity, breadcrumb header, card container, and explicit state disclosure. Destructive actions (like clearing all cookies vs. cache) are separated visually and logically.
   - **LiteCowork Application:** Implemented in `apps/litecowork-ui/src/settings/SettingsView.tsx`, providing a full-featured, two-column settings workspace that maps cleanly to LiteCowork's 187 ledger controls.

2. **Terminal Session Resident Keep-Alive (`sidePaneTerminalSessionRegistry.js`):**
   - In ZCode's `WorkspaceShellLayout.tsx`, closing or switching side pane tabs does not terminate the underlying terminal session. Resident PTY/xterm handles are preserved by `workspaceKey` in a session registry until the workspace itself closes.
   - **LiteCowork Application:** The Workbench terminal pane preserves its session state across panel collapse/tab switches so that long-running builds or commands are not killed by UI navigation.

3. **Authoritative Project Tree vs. Ephemeral UI State (`project-sidebar/CONTRACT.md`):**
   - In ZCode, the Host daemon's `projectService.readTree` is the single source of truth for projects, folders, and CLI status. The UI only stores view preferences (folded/unfolded, order) and uncommitted composer drafts.
   - **LiteCowork Application:** LiteCowork's left sidebar derives active workspaces, roots, and conversations from the authenticated local Operator API projections, ensuring UI state never drifts from daemon reality.

## 9. Review boundary

This review does not amend `PRODUCT.md`, `TRUST.md`, `ENVIRONMENTS.md`, `CHROME-BROWSER-LLD.md`, `API.md`, `EVENTS.md`, schemas or the Conversation Session Supervisor feature package. It records inspiration and proposed UX refinements only. Any promotion into normative contracts requires the relevant domain owner and machine-schema review.

## 10. Related implementation-source audit

The screenshot and UX review above is complemented by [DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md](DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md), which examines the actual ZCode Electron Desktop host/process/plugin-sandbox implementation and the DeepSeek Harness Desktop app. Keep the scopes separate: this document remains the ZCode screenshot/interaction review; the paired audit is the source-level architecture and reuse register. Neither changes LiteCowork's canonical UI, Trust, Runtime, Environment or Effect owners.
