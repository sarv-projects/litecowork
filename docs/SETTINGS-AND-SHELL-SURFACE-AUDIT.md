# Settings and shell surface audit

**Checked:** 2026-11-06 against the active Tauri/React mount path and source files.
**Purpose:** source-backed inventory of what the owner can actually see and invoke; this is not the target Settings design.

## 1. Authority and active render path

- The active Settings page branch in `apps/litecowork-ui/src/App.tsx` renders `<SettingsView ... />` from `apps/litecowork-ui/src/settings/SettingsView.tsx`.
- The exported `SettingsPage` function later in `App.tsx` has no JSX call site in the active source. It is an unmounted legacy/alternate page and is not evidence of the current Settings UI.
- `SettingsView` declares exactly four `SettingsCategory` values and renders four navigation buttons: **General**, **Workspaces**, **Agents**, and **Runtime**. The comments for Browser, Terminal, Security, Skills/MCP and Coworker Defaults are empty placeholders, not rendered categories.
- The target Settings information architecture has 16 categories and the control ledger has 187 named operations across 30 groups. Those are design targets, not current UI coverage.
- The mounted component has no Settings search input, no category search behavior, and no Settings-specific responsive category drawer. Its stylesheet contains no responsive media-query implementation for that navigation.
- The current category defaults to **Workspaces** on mount.

## 2. Mounted Settings controls by category

| Visible category / control | Actual source behavior | Owner path / evidence | Current classification and limitation |
|---|---|---|---|
| General — Interface Theme | `useState("light")`; select updates only the component value | `SettingsView.tsx` | Ephemeral component state only; not persisted and not applied to the application theme |
| General — Layout Density | `useState("comfortable")`; select updates only the component value | `SettingsView.tsx` | Ephemeral component state only; no layout density application or persistence |
| General — Text Scale | `useState("100")`; select updates only the component value | `SettingsView.tsx` | Ephemeral component state only; no global font scaling or persistence |
| General — Reduce Motion | `useState(false)`; checkbox updates only the component value | `SettingsView.tsx` | Ephemeral component state only; no motion preference application or persistence |
| General — shortcut rows | Displays Ctrl+N, Ctrl+W and Ctrl+K labels | `SettingsView.tsx`; source search of `apps/litecowork-ui/src` found no Ctrl/Meta key handler | Display-only claims; no matching shortcut handler was found |
| Workspaces — current Workspace | Selects the active Workspace from the loaded list; refresh button requests list refresh | `SettingsView` props wired from `App.tsx`; Workspace list Tauri/Operator path | Served for list/select/refresh; selection is local view scope, not a Workspace mutation |
| Workspaces — Create Workspace | Form calls the App-owned create operation; disabled while list/create is unavailable or Runtime Operator is not ready | `create_workspace` Tauri command and Workspace Operator route | Served durable creation; busy/list gating and `workspaceError` are visible, but pending-journal retry-specific state is not rendered |
| Workspaces — Workspace details | Displays Workspace ID, status, replication policy and version | Workspace projection passed from App | Read-only projection; displayed policy is not a Settings mutation |
| Workspaces — Workspace Instructions | Edits draft text and submits a new immutable instruction revision; shows revision count/message/history | `create_workspace_instruction_revision` Tauri command and Workspace instruction routes | Served revision write; latest-conflict recovery props are passed but the “use latest” recovery UI is not rendered |
| Workspaces — Workspace Lifecycle | Confirmation UI invokes archive | `archive_workspace` Tauri command and Workspace archive route | Served terminal read-only transition; does not delete local files |
| Workspaces — roots / rename / policy / enrollment | No root list/add/pause/resume/revoke/scan control, Workspace rename control, storage-policy mutation control or Runtime-enrollment control is rendered by the mounted `SettingsView` | Props such as `onUpdatePolicy` and `onEnrollLocalRuntime` are passed but not consumed by this component; root Tauri commands exist separately | Not a Settings capability. Route/command existence elsewhere does not make it a visible Settings action |
| Agents — catalogue and observations | Child `AgentCatalogSettings` renders registry/inventory/profile observations and refresh | `AgentCatalogSettings.tsx`, desktop agent catalogue API | Partial served observation; not general installation/authentication |
| Agents — profile probes | Codex and OpenCode probe actions are passed into the child component | App probe handlers and Tauri profile-probe command | Served local probe for the exact supported profiles; does not establish Conversation turn support |
| Agents — Workspace bindings | Create, enable, disable and default-lead binding callbacks are passed into the child component | AgentBinding Tauri commands and Operator routes | Served for exact binding operations; affects future selection/admission, not a running worker |
| Agents — model/effort/auth/install | No qualified model/effort picker, native sign-in flow or trusted install/update/repair lifecycle is established by this Settings component | Feature 002 and implementation-status evidence | Target or unavailable; descriptor and adapter limitations remain |
| Runtime — Start Local Runtime | Calls the App-owned local Runtime start operation when `runtime.state !== READY` | `start_local_runtime` Tauri command | Served start request; the visibility predicate uses the daemon state rather than the separate `operatorReady` field, so process/readiness disagreement needs an explicit test |
| Runtime — Refresh Status | Calls App-owned Runtime status/lifecycle refresh | `get_runtime_status` and `get_runtime_lifecycle` Tauri commands | Served read/observation |
| Runtime — visible details | Displays daemon state, Operator API state, Runtime ID, incarnation ID, startup policy, recovery state and safe blockers when lifecycle data is available | Runtime status and lifecycle projections | Read-only observation; no startup-policy mutation, stop or drain action is shown |

### Agents category: nested active surfaces

The Agents category is not a single simple catalogue list. It mounts the 1,133-line AgentCatalogSettings component and several child panels. Its current visible sub-surfaces are:

| Nested surface | Visible controls and observations | Owner/source path | Evidence boundary |
|---|---|---|---|
| Coding-agent overview | Refresh catalog; explains installation, profile observation and Workspace binding as separate states | AgentCatalogSettings.tsx | Explanatory UI; none of these states implies a runnable agent |
| 15 curated candidates | Candidate stage, candidate path, installation, Workspace profile observation, session module, handshake and qualified-support fields | Static AGENT_CANDIDATES catalog | Candidate metadata only; not all 15 are remotely discovered or installable |
| ACP Registry distributions | Remote search, paginated results, refresh, cached/stale/observed status, distribution package metadata and supported OS | desktopAgentCatalogApi / ACP Registry read path | Descriptive metadata only; Registry commands/environment values are discarded; Registry presence is not installation or support |
| Local installation inventory | Bounded version-only installation observations for curated agents; auth shown unknown and session not probed | App inventory projection | Read-only; no install/update/repair action |
| Adapter observations | Explicit Codex App Server and OpenCode profile probes, gated on Workspace active, Runtime ready/enrolled and local executable detected | Profile-probe Tauri commands and AgentProfile service | Bounded read-only probe, not inference/session/Task execution |
| Observed profiles | Runtime offer compatibility, process readiness, observed/expiry timestamps, Runtime identity, reported protocol/capabilities and probe diagnostics | AgentProfile observations and Runtime inventory stream | Expiring observation; stale/reconnecting/unavailable stream is stated; compatibility is not session authorization |
| Native controls inspection | Expand/collapse; GET and refresh the AgentControlDescriptor; loading/unavailable/failed/ready states | AgentControlDescriptorPanel, descriptor GET/refresh Tauri commands | Current descriptor producer is unavailable and the route returns 503 DEPENDENCY_UNAVAILABLE; this is an unavailable panel, not a model/effort picker |
| Workspace bindings | Create disabled binding from a fresh compatible offer; enable/disable; lead-eligibility checkbox; binding state and safe auth-reference presence; current Workspace default lead; set another default or clear it | AgentBinding create/enable/disable and Workspace default-lead operations | Served configuration. New binding is disabled. Existing sessions stay attached when a binding is disabled; no active session migration or execution follows |
| Per-binding quota observation | Expand/collapse and request provider observation; show state/source/age/expiry/remaining hint/reset; retry errors | AgentQuotaObservation and get_agent_quota_observation | Read-only provider observation; expired/future values become unknown; no observation is not zero; does not switch lead or represent the global Usage dashboard |
| Subagent profiles | Refresh list; create disabled profile for an enabled binding; revise only an editable disabled profile; duplicate disabled copy; disable; archive; inspect immutable revision history; open Past work performance by task category | desktopDelegationProfileCatalogApi, DelegationProfile Operator routes, DelegationProfileHistory, DelegationProfilePerformance | CRUD/revision/history and read projections exist. No UI action enables a profile for execution; profile status ENABLED means policy selection, not a running worker. No child Attempt/Environment/Trust execution is available |
| Worker-profile performance | Expand/collapse; select one of eight work categories; show sample count, verifier pass rate, typical time, retry/human-intervention rates, observed cost and failure categories | DelegationProfile performance projection | Read-only aggregate projection. No worker execution is currently integrated; absent/insufficient evidence must remain unknown rather than fabricated |
| Worker-profile editor | Profile name, routing description, instructions, fixed-safety-default note, save/cancel, conflict review/load-latest | DelegationProfile create/revision operations | Instructions are guidance, not security controls. Displayed safety defaults are target constraints, not proof of a live Environment/Trust enforcement path |

The child component's own loading/error/empty states are distinct from the parent Settings props. ACP Registry failures retain curated candidates; profile probes require the exact supported preconditions; native controls can report unavailable; quota and performance panels are on-demand; worker-profile mutations retain request IDs and version checks. The outer Settings component combines agentError, agentProfileError and agentBindingError and passes that combined error into the child.

## 3. Important current-source mismatches

- The General controls are not functional application preferences yet. The component state is neither written to localStorage/sessionStorage nor consumed to set root theme, density, text scale or reduced-motion behavior. The source comment claiming localStorage persistence is inaccurate.
- The shortcut labels Ctrl+N, Ctrl+W and Ctrl+K are rendered as text. A source search found no corresponding Ctrl/Meta keydown handlers in the active UI source.
- The Workspaces panel heading says “Workspaces & Roots”, and its description claims file-boundary/storage-policy management, but the mounted panel only selects/refreshes/creates/archives Workspaces, shows replication policy read-only and edits Workspace instructions. It does not render WorkspaceRoot controls or policy mutation. Root operations exposed elsewhere must be documented under their actual owning screen.
- The Agents category heading claims detected OpenCode, Codex, Claude Code and Gemini harnesses, but the active probe actions are only Codex and OpenCode. The 15 curated candidates and ACP Registry distributions are catalogue metadata, not proof those other harnesses are installed, detected or turn-capable.
- The mounted `SettingsView` receives but never reads these props: `runtimeError`, `runtimeLifecycleLoading`, `runtimeLifecycleError`, `workspaceCreatePending`, `onUpdatePolicy`, `policyBusy`, `policyRetryPending`, `latestInstructionsAfterConflict`, `onUseLatestInstructions`, `runtimeBindingError`, `runtimeBindingsLoading`, `runtimeEnrollmentBusy`, `runtimeEnrollmentMessage`, `runtimeEnrollmentError`, `onEnrollLocalRuntime` and `onRefreshRuntimeBindings`. Consequences: Runtime errors/loading are not shown by this component; a pending Workspace-create journal is not represented with a retry-specific label/state; instruction-conflict recovery has no visible “use latest” action; Workspace policy mutation and Runtime enrollment are not visible. These props/handlers must not be counted as user-visible functionality.
- The Runtime card says “Connected (Localhost)”. The authenticated transport is an OS-local Unix socket or Windows named pipe, not a loopback TCP listener. The label is transport-inaccurate.
- App bootstrap first reads Runtime status and attempts automatic start only when `processRunning` is false and the daemon executable is available, then refreshes Runtime and Workspaces. The Settings Start button is a separate manual/fallback action.
- The Runtime subtitle says “Daemon is currently disconnected” whenever `operatorReady` is false, even though the daemon process may be present but its authenticated IPC readiness probe may have failed. The component does not render `runtimeError` to explain that distinction.
- The Runtime subtitle calls the Operator API “serving and ready”. Documentation and UI semantics must preserve that this means Operator serving/readiness only, not Task execution readiness.
- No Browser Control toggle, browser cache/data-clear action, TLS-verified badge, Shell Engine selector, command-approval selector or Windows Credential Store badge was found in the active `apps/litecowork-ui/src` search. Earlier audit claims that these were currently displayed in Settings are stale and must not be repeated.
- Workbench Browser and Terminal are separate Feature 008 surfaces. Their iframe preview and simulated terminal behavior must not be misattributed to Settings or described as real browser/PTY control.

## 4. Unmounted alternate Settings page

`App.tsx` still exports a separate `SettingsPage` function with a larger Runtime/Workspace/Agent layout, including Workspace policy and Runtime enrollment controls. No active JSX call site references it; the active page renders `SettingsView`. Do not count its controls, states or handlers as shipped Settings UI. Its presence is a source-maintenance/duplication concern, not a user-visible feature.

## 5. Other active shell surfaces

| Surface | Active UI/source | Current boundary |
|---|---|---|
| New Chat / Recent Chats | `ConversationsPage` and conversation modules | Durable Conversation flow; one OpenCode dispatch branch; recovery and adapter qualification remain separate |
| Coworkers | Coworker roster/detail/editor, Home Quick Create and Task composer | Revision editor persists lead/delegation/profile allowlist/interaction/failover/context/notification preferences; only the default lead is consumed by Task creation, other policy consumers are incomplete; PAUSED gating is partial/misaligned; presence returns 503; no Coworker-owned Conversation or resident worker |
| Needs You | TaskAttentionPage, ApprovalDetail, UserRequestDetail | Reads persisted attention/request/approval projections; unavailable is not empty |
| Work | Work and Task detail surfaces | Task records/spec/readiness; no admitted Plan/Attempt execution timeline |
| Automations / Routines | AutomationsPage, AutomationEditor, RoutinesPage | TriggerHost can materialize READY Task; it does not execute that Task |
| Library / Artifacts | ArtifactLibrary, ArtifactWorkbench and Resource views | Immutable records and narrow text editing; no general Artifact creation or professional-format engine |
| Discover | DiscoverPage and discovery APIs | Workspace-local skills and observed hosts; LiteSPM lifecycle/Connections remain unqualified |
| Workbench | WorkbenchPanel | Resource preview; Browser iframe preview; simulated Terminal; sample Diffs; Activity branch is not rendered |
| Settings | SettingsView | Four visible categories and the exact controls in §2 only |

## 6. Relationship to target contracts

- `V1-SETTINGS-CONTRACT-LEDGER.md` remains the 187-control normative target ledger; it is not a shipped-control inventory.
- `V1-UI-SCREEN-CONTRACT.md` defines the target Settings information architecture and interaction semantics.
- `SEMANTIC-SERVED-UI-MAP.md` maps current served UI behavior and must agree with this source audit.
- Feature 011 owns Settings shell/navigation and common control-state behavior. Domain owners retain their own Settings sub-panels and operations.
- Feature 008 owns Browser/Terminal/Computer Workbench semantics; Feature 002 owns Agent lifecycle; Workspace, Runtime and Trust operations remain with their canonical owners.

## 7. Acceptance requirements for reconciliation

- Every visible Settings control has an exact owner, scope, operation, status, loading/pending, success, error and recovery contract.
- Local preferences must actually apply and persist if the product promises them; otherwise they must be disabled or described as preview-only.
- Shortcut labels must match registered handlers.
- Target-only Settings categories must not appear as active controls.
- WorkspaceRoot and Runtime enrollment actions must be mapped to their actual UI surface, not inferred from available Tauri commands.
- Runtime UI copy must distinguish process state, authenticated Operator serving, lifecycle availability and Task execution readiness, and must name the transport accurately.
- Packaged UI keyboard/focus, responsive layout, accessibility and real owner-operation behavior remain release qualification gates.

## 8. Related contracts

`V1-UI-SCREEN-CONTRACT.md` · `V1-UI-ACTION-STATE-LEDGER.md` · `V1-SETTINGS-CONTRACT-LEDGER.md` · `DESKTOP-SHELL-LLD.md` · `SEMANTIC-SERVED-UI-MAP.md` · `IMPLEMENTATION-STATUS.md` · Feature 011 · Feature 008
