# Desktop Shell, Settings, Sidebars, Interaction, Animation LLD (V1)

Status NORMATIVE TARGET. Owners E02/E03/E08/E09/E10/E13. Authority: EXPERIENCE, DESIGN-SYSTEM, MOTION, PRESENTATION-RUNTIME, UI delivery. No speculative control is active until backed by Operator/UI ownership, entitlement and USER qualification.

## Window and navigation anatomy

~~~text
AppWindow
  GlobalTitleBar(WorkspaceSwitcher, Search, QuickEntry, Notifications, RuntimeStatus, WindowControls)
  AppGrid
    LeftRail(expanded or compact, resizable)
      NewSession, Search, RecentConversationList
      CoworkerRoster(expandable, per-item menu, add)
      Work, NeedsYou(count real), Automations, Library, Discover
      Settings, diagnostics/profile
    MainPane
      RouteOutlet(Home/Conversation/Coworker/Work/NeedsYou/Library/Automations/Discover/Settings)
      ConversationView when chat route
        ConversationHeader(agent/coworker, task state, continuity)
        SemanticMessageStream
        StickyAgentComposer
    RightWorkbench(resizable, closable, max, history)
      Start, Browser, Computer, Terminal, Artifact/Office, Code/Diff,
      Image/PDF, MCP App, Generated UI, Task details
  GlobalOverlays(command palette, native palette, dialogs, toast, progress)
  AccessibilityLiveRegion
~~~

One right Workbench only; do not build another competing right "Inspector" pane. Inspector is a disclosure/tab in details. No Cowork versus Code top-level split; Code is a task preset. Clicking a Coworker opens its exact owned chat or an honest unavailable state, never another chat labeled as that Coworker.

## Shell state model

~~~ts
type DesktopLayoutState = {
  workspaceId:string; route:string; left:{mode:"OPEN"|"COMPACT"|"HIDDEN";width:number};
  right:{open:boolean;widthRatio:number;maximized:boolean;
    active:{kind:WorkbenchKind;objectId:string|null;revision?:string}|null;
    back:WorkbenchEntry[];forward:WorkbenchEntry[]};
  overlays:{kind:string;ownerId:string;openedFrom:string|null}[];
  density:"COMFORTABLE"|"COMPACT";theme:"SYSTEM"|"LIGHT"|"DARK";
  reducedMotion:boolean; textScale:number;
  focusReturnId:string|null; layoutEpoch:number;
};
type PaneStatus="READY"|"EMPTY"|"LOADING"|"PARTIAL"|"OFFLINE"|"NO_ACCESS"|"REVOKED"|"ERROR"|"STALE";
~~~

Persist user layout preferences by Workspace and window; do not persist grant/lease/authorization. Restore only after revalidating object access. Changing Workspace increments layoutEpoch; stale async results cannot render. An unsaved right-pane draft blocks destructive close/navigation and supplies Save/Discard/Stay. Application close while uncertain publish has locked retry state, not fake discard.

## Geometry and responsive behavior

- Left rail compact icon navigation with usable accessible labels. Open width user-adjustable; independent of right Workbench. New first-run shows Discoverable routes without inventing data.
- Right Workbench closed for new session; last explicit choice restored per Workspace. First open target ~42% width with minimum token limits, max ~72%; horizontal divider keyboard-operable and accessible; maximize preserves app chrome and exact prior ratio.
- Below 900 CSS pixels, right Workbench becomes a full-surface route/sheet, not squeezed beside chat. Narrow mobile-like view keeps accessible composer. At 200/400% zoom no essential action clipped.
- At most one high-priority modal; secondary menus/popovers dismiss safely. Overlays portal to window root to avoid clipping; focus restore after close; no loss of typed draft.
- Central chat scroll anchors: if user at bottom, stream keeps within threshold; if scrolled up, do not jump on new tokens; show "New messages" affordance. Large message virtualization retains stable anchor and accessible reading order.
- Split drag saves on commit/debounce; during drag content resize is throttled. Maximize/restore and route changes cannot overlay native browser video incorrectly. Handle DPI and multiple monitors.

## Interaction state reducer API (proposed frontend modules)

| Function | Inputs -> result | Invariant |
|---|---|---|
| routeTo | destination, Workspace epoch, dirty guard -> navigation | Keep unsent edits and correct owner identity |
| selectCoworkerChat | coworkerId, Workspace -> exact Conversation or empty | No unrelated fallback; cancel prior request |
| openWorkbench | object kind/ref, reason(USER_EXPLICIT/RESTORED) -> pane | Background results don't steal focus |
| pushWorkbenchHistory | active object -> bounded back/forward | Avoid duplicate adjacent entries; auth on revisit |
| resizeWorkbench | ratio/key delta -> clamped width | pointer/keyboard; persists on release |
| closeWorkbench | unsaved state -> closed or confirm | re-focus opener; stop no Task implicitly |
| toggleWorkbenchMax | prior ratio -> fullscreen/restore | preserve layout and scroll |
| hydrateLayout | saved prefs + permission probes -> safe state | show Start on stale object; never restore old grants |
| openGlobalPalette | shortcut and surface priority -> overlay | native composer autocomplete takes priority when open |
| deriveSidebarCounts | authoritative projections -> counts/UNKNOWN | don't display zero for unavailable API |
| takeFocusForAlert | priority, user action -> focus | only for critical action; stream never autofocus |
| showToast | persisted receipt/operation -> deduped toast | never toast success before accepted receipt |
| handleGlobalStop | activity/Task/Automation scope -> confirmation | stop new admissions vs existing effects distinct |
| useScopedRequest | workspaceId, selectionEpoch, abort -> response | drop stale results on Workspace/Coworker switch |
| commitThemeSettings | OS preference/user choice -> tokens | no flash; high contrast supported |
| ensureAccessibleOverlay | modal focus root/trigger -> focus cycle | Esc safe; ARIA; no trapped focus after failure |

## Left sidebar UI control catalog

New session; conversation search/recent/pin/rename/archive/delete when supported; Coworker roster create/rename/settings/pause/archive; Work filters by active/needs you/finished; Needs You actual pending count; Automations list and create; Library Resources/Artifacts search; Discover packages/Connections; Settings navigation; Workspace switch; collapsed icons with keyboard accessible labels; global status. Every control needs loading, empty, error, permission-denied, stale-state and offline branches and deep-link validation.

Conversation search is permission-scoped and count-capped/virtualized; a missing search index differs from "no results." Archived Coworkers never route to active edit by accident. Delete/Archive changes do not cascade to independent Tasks, Artifacts or Memory without explicit impact approval.

## Settings IA (explicit owner map)

~~~text
Settings
 General: language, appearance, density, zoom, keybindings, notification quiet hours
 Workspaces: list/create/rename/roots/instructions/archive/default lead
 Agents: ACP/live registry + AgentModules, install/update/repair, native auth,
   AgentBinding, native config, default lead, session/model/effort/mode controls
 Local Models: discovery, compatibility shown only with specific agent; no fake provider
 Subagents: delegation profiles, worker capabilities, limits/cost/parallelism, version pinning
 Connections & Tools: LiteSPM-owned MCP, skills, apps, connectors, installations,
   connected accounts, per-Coworker assignments and exact permissions
 Browser & Computer: qualified Chrome/managed profiles/import/attach/takeover/site grants
 Memory & Knowledge: Workspace/Coworker-scoped sources, extraction controls, revoke
 Automations: triggers/overlap/retry/quiet windows/occurrence history
 Runtime: daemon status, startup/background policy, hardware, environment, cleanup
 Security & Privacy: data egress, grants, OS keychain, credentials presence, retention
 Notifications: classes, per-channel delivery, quiet hours, pending Needs You
 Appearance & Accessibility: theme, contrast, reduced motion, keyboard
 Storage & Backup: location, quotas, report, import/export and backup restore
 Usage & Monitoring: per-session/task/agent totals, price confidence, diagnostics, caches, resilience
 Terminal: default shell/profile, scrollback, integration, keep-alive, security/paste and UI
 About/Diagnostics: build, API/schema versions, OS, logs redacted, health, support bundle
~~~

Implementation may group panels visually but cannot hide necessary per-agent setup under a generic Provider picker. Each setting has canonical owner, current value/version, validation, cancel/reset, saved/error/pending feedback, keyboard flow and scope label (global/Workspace/Coworker/AgentBinding/Runtime). Some settings are V1 goals requiring new contracts; do not present speculative toggles as working.

## Per-control settings transitions

- Agent install/update: show version/channel/source/digest, compatibility/license and expected storage; download/verify/launch in owned installer boundary; preserve previous working version until replacement healthy; repair and rollback must not destroy native user configuration.
- Agent auth: prefer official native login. Secure API key entry uses native OS keystore broker, never plain frontend persistence. Auth canceled/expired is distinct from provider unavailable or unsupported option.
- Model/effort options: source from selected agent descriptor, never global hardcoding. New-session-only changes disclose when effective.
- Connection install vs activation vs authorization vs assignment vs invocation distinct. Deleting assignment does not uninstall package or revoke a different Workspace.
- Subagent profiles versioned; active Tasks retain old worker pins; configured price/quotas are estimates unless observed provider readings.
- Notifications/automation toggles do not silently authorize a standing action. Enabling a schedule reviews scope/when/where/budget/approvals and persists trigger readiness.
- Runtime "Start" means daemon serving, not execution-ready until actual policy/provider gates pass. "Stop safely" drains and preserves ambiguous effects.
- Workspace archive lists blocking Tasks/Automations/leases and preserves owner documents, using optimistic revision checks.
- Browser import and storage cleanups show dry-run and confirmation; never erase native Chrome default user data.

## Right pane surface state and actions

Start shows Workspace Files, Browser, Computer, Terminal if qualified, Recent Outputs if real. Tabs/surface navigation use controlled history, not uncontrolled nested independent sidebars. Artifact viewer and editor share exact version/dirty state. Browser header includes actual browser owner mode and Take/Return. Computer shows current display target, permissions, privacy/capture status and stop control. Terminal shows provider, working directory, output retention, action permissions. MCP App/Generated UI uses sandboxed host capability bindings and fallback; no arbitrary untrusted script execution. Inspector reveals exact Task/Attempt/Effect evidence on demand, without intimidating ordinary chat.

## Microinteraction and animation specification

Use existing MOTION tokens: instant 80ms, fast 120ms, normal 180ms, slow 280ms and prescribed easing. Semantic motion MUST be tied to real state transitions: rail expansion, Workbench open/close/max, page replacement, dropdown, command palette, new message commit, approval status, Task dispatch, blocker appearance, takeover epoch, Artifact version publish. No moving cursor implying an agent used a mouse when using structured APIs; no imaginary typing; no fake work percentages or ETA.

- Layout transitions use transform/opacity on composited layer where practical; avoid animating expensive large message lists or third-party browser frames. Respect prefers-reduced-motion and app setting: snap state without implying ongoing action.
- Stream text incrementally as actual content arrives; do not animate each character from stored messages. Reconnect restores committed projection without replaying animations.
- Overlay exit restores focus at state commit, not before the animation completes; stale animation callbacks carry layoutEpoch and are ignored.
- Skeletons only for bounded pending reads; timeout gives explicit unavailable state. Spinners can accompany true in-progress backend actions only.
- Error moves focus once and announces stable accessible summary; no flashing/shaking essential to understanding.
- Touch/pointer hit targets >= 44px for compact breakpoints; hover-only functionality always has click/keyboard equivalent.

## UI event contracts

Events are local frontend reducer actions until promoted to canonical API: NavigationRequested/Committed/BlockedByDraft; WorkbenchOpened/Closed/HistoryChanged; AgentSelectionRequested/Resolved; ComposerEligibilityChanged; NativePaletteOpened/Committed; PaneResizeCommitted; TakeoverRequested/Committed; RichContentFallbackSelected; ProviderConnectionRepairOpened; RoutineEnabled/Paused; AuthRepairRequested. Each has correlation id, Workspace epoch, revision and source control; only accepted durable domain actions cause canonical Events. Never log user draft content or secrets in UI telemetry.

## Visual/quality gates

- Snapshot images at desktop widths 1440/1280/1024/900/600/400; 100/200/400% zoom; light/dark/high-contrast; English + long RTL/Unicode; active long streams, deeply nested tools, large tables, 50 Coworkers, 100 connections.
- User can create chat, switch between two agents mid-draft, use native / @, open Workbench, take over browser, answer Needs You, restore draft and navigate Settings keyboard-only.
- Verify actual shell in Windows/macOS/Linux builds; browser/video/terminal focus, native context menus, resize and screen-reader announcements require real system tests.
- Every documented control crosswalks to a UI component, backend receipt/owner or an explicit unavailable/repair state. A greyed inert button without explanatory reason is a defect.


## Full Settings controls and data-flow acceptance ledger

Each row is a required user-visible operation, not an assumption that an existing endpoint already exists. The control owner MUST bind to a typed domain service/Operator command, record idempotency, error/rollback and scope. If backend contract not served, show explicit setup/unavailable reason rather than a clickable sham.

| Category | Controls and handlers (implementation target) | Failure, safety, visual state |
|---|---|---|
| Global | changeAppearance, changeLanguage, setDensity, setReducedMotion, setTextScale, resetShortcuts | OS theme race, RTL, 400% zoom, bad shortcut conflicts |
| Workspaces | createWorkspace, renameWorkspace, selectWorkspace, archiveWorkspace, switchDefaultLead, changeReplicationPolicy | expected-version CAS, archived read-only, dirty draft guard, local/cloud policy truth |
| Roots | selectFolder, addWorkspaceRoot, setWatchPolicy, pauseRoot, rescanRoot, revokeRoot, viewIndexedResources | symlink and unauthorized path, offline root, canceled scan, changed revision |
| Instructions | editWorkspaceInstructions, compareRevisions, saveNewRevision, restoreAsDraft | version conflict, oversized UTF-8, unchanged draft, source provenance |
| Agents catalog | searchAgentRegistry, filterReady, inspectAgentModule, refreshAgentInventory, openNativeConfig | stale registry, unsupported OS, no module, expired offer |
| Agents install | previewAgentInstall, installAgent, updateAgent, repairAgent, selectVersion, rollbackInstall | signature/source/version, disk limit, busy session, native config preservation |
| Agents auth | openNativeSignIn, addSecureKeySlot, testNativeAuth, reauthenticate, signOutNative | native owner scope, failed OAuth, secret not persisted to React, revoked session |
| Agents defaults | createBinding, disableBinding, setWorkspaceDefault, setCoworkerDefault, setModelDefault, setEffortDefault, setModeDefault | agent-dependent descriptor, stale option, active Attempt pinning, new-session limit |
| Local models | discoverLocalServer, listAvailableModels, pairCompatibleAgent, checkHardwareReadiness | inventory only; no global provider abstraction; missing runtime/VRAM |
| Subagents | createWorkerProfile, cloneProfile, setWorkerCapabilities, setParallelLimit, setBudgets, changeRouting, archiveProfile | capability checks, quota UNKNOWN, preserved Task revision |
| Connections | searchCatalog, filterKind, previewPackage, installPackage, connectAccount, assignCoworker, repairConnection | install != activate != grant, trusted publisher, remote auth, missing dependency |
| MCP | addLocalServer, addRemoteServer, showTransport, testServer, disconnectServer, setToolAllowlist | private process args redacted; weak server auth; stale manifest |
| Skills | previewSkill, inspectManifest, testSkill, addSkill, updateSkill, removeSkill | untrusted instructions, changed digest, non-authoritative text |
| Browser | discoverChrome, selectChromeChannel, createManagedProfile, previewBookmarkImport, commitImport, connectExistingChrome, detachSession | real Chrome version, consent, imported bookmarks not logins, screen fidelity |
| Browser grants | setSiteAccess, setObservePermission, setDownloadPermission, resetChromeProfile, deleteManagedProfile | per-origin scope, evidence, active lease/drain, precise deletion confirmation |
| Computer | selectDisplayTarget, setScreenObservationPolicy, grantDesktopUse, revokeDesktopUse, calibrateInput | OS permission, locked screen, cross-display DPI, wrong active window |
| Coworker identity | createQuickCoworker, editNameAndRole, chooseChief, pauseNewWork, pauseAllSafely, resumeCoworker, archiveCoworker | current tasks survive where pinned, no auto-connect, version conflicts |
| Memory | setLearningMode, listMemory, inspectSource, correctMemory, resolveConflict, forgetSource, deleteMemory | private default, scope, pending external purge, temporary chat exclusions |
| Knowledge | attachSource, selectFolderOrCloudResource, previewIndex, pauseIndex, removeSource, chooseRetrievalScope | source revision pin, unsupported parser, stale index, no automatic egress |
| Goals | createGoal, updateGoal, relateTask, inspectEvidence, archiveGoal | progress derived only from real sources; immutable user steering |
| Routines | createDraftRoutine, editRoutine, saveRevision, runOnce, inspectRuns, archiveRoutine | save != run, revision pin, missing resource policy |
| Automations | createAutomation, previewSchedule, setTrigger, setTimezone, chooseOverlapPolicy, reviewGrants, enable, pause, runNow, archive | DST, asleep host, missing connection, duplicate trigger, budget limits |
| Notifications | setQuietHours, muteClass, setDeliveryChannel, viewQueue, openRequest, acknowledge | event dedupe, quiet behavior, NeedYou count accurate |
| Runtime | inspectHealth, startDaemon, setStartupMode, stopSafely, inspectRecovery, viewActiveEnvironments | serving != execution qualified, drain in progress, writer quiescence unknown |
| Security | viewActiveGrants, revokeGrant, inspectCredentialPresence, rotateLocalSecret, viewEgressScope, setRetentionPolicy | no secret bytes, pending revoke on active Effect, version fences |
| Backup | previewBackup, exportWorkspace, verifyManifest, importBackup, previewRestore, confirmRestore | encryption ownership, collision, stale schema, no silent overwrite |
| Storage | inspectStorageUsage, previewSafeCleanup, pruneAgentBuildDebris, manageArtifactRetention | owner file vs generated build, active processes, no native session deletion |
| Usage & Monitoring | inspectUsageTotals, filterUsageDateRange, filterUsageAgent, inspectUsageConfidence, inspectCostPriceSource, inspectQuotaHealth, inspectFailureTimeline, exportUsageReport, setTelemetryRetention, setDiagnosticSampling | native opaque usage UNKNOWN, estimates distinct from billed; currency separation; no raw secrets |
| Terminal | inspectShellProfiles, chooseDefaultShellProfile, previewCustomShellPath, validateCustomShellPath, setTerminalScrollback, setTerminalRetention, setShellIntegration, setTerminalPasteConfirmation, viewTerminalHealth, clearTerminalViewport | profile OS/Runtime pin, PTY owner, control epoch, no stealth command replay |
| About/Diagnostics | showVersion, exportRedactedLogs, runProviderDiagnostic, copyBuildInfo, openSupportDocs | sanitized, time-bound, no fake provider test success |

**Settings page controller pattern**:

~~~ts
type SettingsMutation<T> = {
  // UI scope is a *selection hint*, never the authenticated actor identity.
  scope:{workspaceId?:string;coworkerId?:string;bindingId?:string;runtimeId?:string};
  operation:string; draft:T; expectedVersion:number; idempotencyKey:string;
};
async function submitSettingsMutation<T>(mutation:SettingsMutation<T>) {
  const validation = validateClientSchema(mutation);
  if (!validation.ok) return showFieldErrors(validation);
  const decision = await preflightRequiredPermissionAndImpact(mutation);
  if (!decision.allowed) return showRepairOrConfirm(decision);
  // Operator uses authenticated OS-peer/HTTP Principal, X-Workspace-ID, If-Match,
  // and a stable Idempotency-Key mapped to domain RequestId. IPC request_id is correlation only.
  const response = await scopedOperatorCommand(mutation);
  if (response.isAmbiguous) return lockPendingAndReconcileSameIdempotencyKey(mutation);
  if (response.isConflict) return preserveDraftAndShowCurrentVersion(response);
  return refreshExactOwnerAndConfirmReceipt(response);
}
~~~

Every row has busy/empty/unavailable/error/permission denied/success and optimistic conflict states, keyboard labels, focus strategy and tests. A UI change can update a local preference without a server write only if that choice grants no authority and is explicitly scoped to the current window.

## Fine-grained left rail and right Workbench behavior

| Input | Context | Deterministic expected result |
|---|---|---|
| Ctrl/Cmd+K | outside native editor autocomplete | global route/Resource search with appropriate authorization; never executes slash |
| Type / in composer | selected native harness declares it | filtered native slash palette; custom command collision preserves native behavior |
| Type @ in composer | native descriptor declares reference kinds | @file or @agent palette sourced from compatible harness; no global guess |
| Ctrl/Cmd+Enter | eligible composer, IME idle | immutable turn submit exactly once |
| Enter | palette open | choose highlighted entry; don't send |
| Escape | nested palette/dialog | close highest safe overlay; if pending side effect cannot discard |
| New session | left rail | ordinary chat with selected default binding, Workbench closed unless persisted preference |
| Coworker click | left roster | exact owner-revision chat, or honest owner API unavailable |
| + new Coworker | left roster | Quick Create; not forced multi-page wizard |
| Collapse rail | desktop shell | icons retain visible focus/tooltip and accessible names |
| Open Browser | right Start | require qualified provider and explicit mode; no unauthorised auto-open |
| Open file | message or Library | authorized exact version in Workbench; unsaved draft guard |
| Take control | live Browser/Computer | wait for epoch-fenced ownership transfer; pending state until receipt |
| Return to agent | browser/computer | fresh state capture/revalidation, old actions invalidated |
| Workbench back | panel history | reopen exact authorized object; no navigation if dirty unresolved |
| Workbench close | panel header | chat restored, active Task not automatically stopped |
| Workbench maximize | panel header | content area without global app chrome loss, restore exact ratio |
| Resize divider | pointer/key | immediate visual change, debounce saved preference, accessible percentage |
| Archive Workspace | Settings | show dependencies and explicit confirmation; exact CAS and current Task blockers |
| Pause all safely | Coworker/Global | fence new admission and request running work safe stop; actual pending status |

## Animation event timeline examples

~~~text
Agent picker clicked -> 80ms menu open -> selected binding -> controls in LOADING
fresh descriptor acknowledged -> one atomic 180ms crossfade of dependent model/effort/commands
draft incompatible -> static persistent inline warning, no further animation
user resolves tokens -> real ELIGIBLE state, send enabled

Workbench Open pressed -> permission resolution -> pane shell transition 180ms
content may still be LOADING, with state announcement; no background focus steal
drag resize -> no uncontrolled list animations; upon release save ratio
close -> dirty guard -> 180ms pane transition + restore opener focus

Browser Take control pressed -> label TRANSFER_PENDING (no fake cursor)
lease epoch committed -> 120ms status indicator change, human input becomes enabled
Return -> fresh observation/check -> owner AGENT, stopped stale actions not replayed

Task starts -> persisted RUNNING/Attempt event -> activity appears once
reconnect -> authoritative snapshot replaces provisional frames with no repeated animation
Task finishes -> verified result event -> final status transition; no fake confetti
~~~

Transitions must be interruptible (Workspace/route change cancels in-flight animation), driven by state rather than timeouts, and atomic with actual ARIA status updates. Focus is not moved to streaming text. Reduced-motion variant replaces fades/slides with instantaneous state changes while maintaining the same semantic announcements.

## Audit-completed control crosswalk

The 30 Settings groups and 187 named operations above now have a one-per-control owner/scope/mutation-class/failure crosswalk in [V1-SETTINGS-CONTRACT-LEDGER.md](V1-SETTINGS-CONTRACT-LEDGER.md). Non-Settings titlebar, left rail, composer, main pane, right Workbench, browser, computer and overlay actions are mapped to 35 UI interaction groups in [V1-UI-ACTION-STATE-LEDGER.md](V1-UI-ACTION-STATE-LEDGER.md). These ledgers give design accountability, not assurance that every handler is served. Any existing canonical Operator, Trust and Task contract overrides inferred handler names.

## Usage, memories, terminal and agent registry surfaces — V1 expansion

Render read-only Usage/Health/Logs/Cache summaries and cost confidence from [V1 Telemetry/Usage LLD](V1-TELEMETRY-USAGE-RESILIENCE-LLD.md); never report missing native usage as $0. Agent cards follow [V1 Agent Discovery LLD](V1-AGENT-DISCOVERY-AND-CATALOG-LLD.md) for official bundled logos and independent status chips. Memory UI follows [V1 Agent Memory Fast Path](V1-AGENT-MEMORY-FAST-PATH-LLD.md); native agent memory remains owned by the harness. Right Workbench terminal follows [V1 Integrated Terminal LLD](V1-INTEGRATED-TERMINAL-LLD.md): shell picker, PTY tabs/splits, OS/Environment badge, Stop/Detach distinct from Task Stop, consented control transfer and truthful reconnect. Inspired by OmniRush/OmniRoute screenshots as reference only; no copied product UI. New controls are target until served/qualified.
