# Semantic Served-Handler ↔ UI Map

**Status:** TARGET BASELINE COMPLETE — SERVED operationIds still open until source recovery (BT-0.4 remainder)  
**Updated:** 2026-11-01  
**Purpose:** for each **user-visible control or read surface**, record whether a **served** Operator handler exists, the **OpenAPI operationId**, projection/stream source, and UI state when **target-only** or **unavailable**.

This is not pixel spec. It implements the “UI renders state + sends commands” rule ([UI-CORE-TYPED-CONTRACTS.md](UI-CORE-TYPED-CONTRACTS.md)).

## Status vocabulary

| Status | UI rule |
|---|---|
| **SERVED** | Control enabled when preconditions pass; typed success/error |
| **TARGET** | Documented in OpenAPI/prose; no handler in active baseline |
| **UNAVAILABLE** | Honest disabled + reason + repair link |
| **LOCAL_ONLY** | Reducer/layout pref; no domain mutation |
| **NATIVE** | Harness/OS flow; not Operator HTTP body from WebView |

Fill **Evidence** with archive served inventory path or fresh qualification run when source returns.

---

## Shell surfaces (summary)

| Surface | Primary operations | Map status | Notes |
|---|---|---|---|
| New Chat / Create Conversation | Conversation create + admission | **TARGET / fail-closed** | SDD 001; historical 503 on send |
| Recent chats mutate | rename/pin/archive/delete | **TARGET** | Screen contract gap list |
| Composer Send/Stop | turn submit/cancel | **TARGET** | 001 |
| Operator stream | TURN_DELTA, TURN_SETTLED | **TARGET** | Producers not mounted in baseline |
| Global search / palette | search routes | **TARGET** | Screen contract gap |
| Needs You | approvals, user requests | **PARTIAL TARGET** | List vs count badge historical |
| Work Task controls | steer/pause/cancel/effects | **TARGET** | API target-only |
| Discover install/connect | LiteSPM + Connection | **TARGET** | Partial read served in historical snapshot |
| Workbench Browser/Terminal/Computer | provider routes | **TARGET** | P7 |
| Settings (187 controls) | per ledger | **MIXED** | See § Settings groups below |

**Research:** complete row-level map requires recovered source + served-route inventory rerun (BT-0.4 in [LITECOWORK-BUILD-TASKS.md](LITECOWORK-BUILD-TASKS.md)). Historical reference: [archive served capability inventory](archive/legacy-implementation-c7e9c05/V1-SERVED-CAPABILITY-INVENTORY.md) — **not current truth**.

---

## Settings groups — mapping template

For each group, extend this table during BT-0.4. Initial pass: **all rows default TARGET** until fresh inventory unless archive explicitly marked served.

| Settings group | Ledger rows | Default status | Canonical API owner |
|---|---|---|---|
| Global | 6 | LOCAL_ONLY | DESIGN-SYSTEM |
| Workspaces | 6 | TARGET | API Workspace routes |
| Roots | 7 | TARGET | WORLD-RESOURCES, API |
| Instructions | 4 | TARGET | DATA-MODEL, STORAGE |
| Agents catalog | 5 | MIXED historical | AGENT-CONTROL |
| Agents install/auth/defaults | 17 | MIXED historical | AGENT-CONTROL |
| Local models | 4 | TARGET | AGENT-CONTROL |
| Subagents | 8 | TARGET | DELEGATION |
| Connections | 7 | TARGET | CONNECTORS, LiteSPM |
| MCP / Skills | 8 | TARGET | CAPABILITY-FABRIC |
| Browser / grants | 11 | TARGET | CHROME-BROWSER-LLD |
| Computer | 5 | TARGET | ENVIRONMENTS |
| Coworker identity | 6 | TARGET | COWORKERS-TARGET |
| Memory / Knowledge / Goals | 15 | TARGET | LEARNING, CONTEXT |
| Routines / Automations | 12 | MIXED historical | AUTOMATION |
| Notifications / Runtime / Security | 14 | TARGET | RUNTIME-LIFECYCLE, TRUST |
| Backup / Storage / Usage / Terminal / About | 22 | TARGET | respective owners |

**Action:** when implementing SDD 011, replace **TARGET** with **SERVED** + operationId per control or mark **UNAVAILABLE** with `failure_code`.

---

## Cross-links

| Control domain | Ledger | UI behavior spec |
|---|---|---|
| Composer | V1-UI-ACTION-STATE-LEDGER | specs/001/.../ui-behavior.md |
| Settings | V1-SETTINGS-CONTRACT-LEDGER | [specs/011 ui-behavior](../specs/011-desktop-shell-ipc-settings-runtime/contracts/ui-behavior.md) |
| Full shell | V1-UI-SCREEN-CONTRACT | DESKTOP-SHELL-LLD |

---


## Settings controls — row baseline (2026-11-01)

Every ledger control is listed. **SERVED is not claimed.** Docs-first checkout has no active Operator handlers. `operationId` stays blank until a fresh served inventory names one. Class comes from [V1-SETTINGS-CONTRACT-LEDGER.md](V1-SETTINGS-CONTRACT-LEDGER.md).

| Control | Group | Map status | Ledger class | Authority | operationId |
|---|---|---|---|---|---|
| changeAppearance | Global | **LOCAL_ONLY** | LOCAL_PREF | Local UI/OS preference | |
| changeLanguage | Global | **LOCAL_ONLY** | LOCAL_PREF | Local UI/OS preference | |
| setDensity | Global | **LOCAL_ONLY** | LOCAL_PREF | Local UI/OS preference | |
| setReducedMotion | Global | **LOCAL_ONLY** | LOCAL_PREF | Local UI/OS preference | |
| setTextScale | Global | **LOCAL_ONLY** | LOCAL_PREF | Local UI/OS preference | |
| resetShortcuts | Global | **LOCAL_ONLY** | LOCAL_PREF | Local UI/OS preference | |
| createWorkspace | Workspaces | **TARGET** | OPERATOR_CAS | WorkspaceService | |
| renameWorkspace | Workspaces | **TARGET** | OPERATOR_CAS | WorkspaceService | |
| selectWorkspace | Workspaces | **LOCAL_ONLY** | LOCAL_SELECTION_EPOCH | WorkspaceService | |
| archiveWorkspace | Workspaces | **TARGET** | OPERATOR_CAS | WorkspaceService | |
| switchDefaultLead | Workspaces | **TARGET** | OPERATOR_CAS | WorkspaceService | |
| changeReplicationPolicy | Workspaces | **TARGET** | OPERATOR_CAS | WorkspaceService | |
| selectFolder | Roots | **LOCAL_ONLY** | OS_PICKER_NO_PERSIST | ResourceService | |
| addWorkspaceRoot | Roots | **TARGET** | OPERATOR_CAS | ResourceService | |
| setWatchPolicy | Roots | **TARGET** | OPERATOR_CAS | ResourceService | |
| pauseRoot | Roots | **TARGET** | OPERATOR_CAS | ResourceService | |
| rescanRoot | Roots | **TARGET** | OPERATOR_CAS | ResourceService | |
| revokeRoot | Roots | **TARGET** | OPERATOR_CAS | ResourceService | |
| viewIndexedResources | Roots | **TARGET** | SCOPED_READ/OBSERVATION | ResourceService | |
| editWorkspaceInstructions | Instructions | **TARGET** | OPERATOR_CAS | WorkspaceInstructionRevision owner | |
| compareRevisions | Instructions | **TARGET** | SCOPED_READ/OBSERVATION | WorkspaceInstructionRevision owner | |
| saveNewRevision | Instructions | **TARGET** | OPERATOR_CAS | WorkspaceInstructionRevision owner | |
| restoreAsDraft | Instructions | **TARGET** | OPERATOR_CAS | WorkspaceInstructionRevision owner | |
| searchAgentRegistry | Agents catalog | **TARGET** | SCOPED_READ/OBSERVATION | AgentModuleRegistry | |
| filterReady | Agents catalog | **TARGET** | SCOPED_READ/OBSERVATION | AgentModuleRegistry | |
| inspectAgentModule | Agents catalog | **TARGET** | SCOPED_READ/OBSERVATION | AgentModuleRegistry | |
| refreshAgentInventory | Agents catalog | **TARGET** | QUALIFIED_READ | AgentModuleRegistry | |
| openNativeConfig | Agents catalog | **NATIVE** | NATIVE_CONFIG_OPEN_WITH_CONSENT | AgentModuleRegistry | |
| previewAgentInstall | Agents install | **TARGET** | SCOPED_READ/OBSERVATION | AgentLifecycleService | |
| installAgent | Agents install | **NATIVE** | NATIVE_MUTATION | AgentLifecycleService | |
| updateAgent | Agents install | **NATIVE** | NATIVE_MUTATION | AgentLifecycleService | |
| repairAgent | Agents install | **NATIVE** | NATIVE_MUTATION | AgentLifecycleService | |
| selectVersion | Agents install | **NATIVE** | NATIVE_MUTATION | AgentLifecycleService | |
| rollbackInstall | Agents install | **NATIVE** | NATIVE_MUTATION | AgentLifecycleService | |
| openNativeSignIn | Agents auth | **NATIVE** | NATIVE_MUTATION | Native harness auth/OS SecretStore | |
| addSecureKeySlot | Agents auth | **NATIVE** | NATIVE_MUTATION | Native harness auth/OS SecretStore | |
| testNativeAuth | Agents auth | **TARGET** | QUALIFIED_PROVIDER_PROBE | Native harness auth/OS SecretStore | |
| reauthenticate | Agents auth | **NATIVE** | NATIVE_MUTATION | Native harness auth/OS SecretStore | |
| signOutNative | Agents auth | **NATIVE** | NATIVE_MUTATION | Native harness auth/OS SecretStore | |
| createBinding | Agents defaults | **TARGET** | OPERATOR_CAS | AgentBinding/Coworker/Workspace owner | |
| disableBinding | Agents defaults | **TARGET** | OPERATOR_CAS | AgentBinding/Coworker/Workspace owner | |
| setWorkspaceDefault | Agents defaults | **TARGET** | OPERATOR_CAS | AgentBinding/Coworker/Workspace owner | |
| setCoworkerDefault | Agents defaults | **TARGET** | OPERATOR_CAS | AgentBinding/Coworker/Workspace owner | |
| setModelDefault | Agents defaults | **TARGET** | OPERATOR_CAS | AgentBinding/Coworker/Workspace owner | |
| setEffortDefault | Agents defaults | **TARGET** | OPERATOR_CAS | AgentBinding/Coworker/Workspace owner | |
| setModeDefault | Agents defaults | **TARGET** | OPERATOR_CAS | AgentBinding/Coworker/Workspace owner | |
| discoverLocalServer | Local models | **TARGET** | SCOPED_READ/OBSERVATION | Qualified agent/Runtime offer | |
| listAvailableModels | Local models | **TARGET** | SCOPED_READ/OBSERVATION | Qualified agent/Runtime offer | |
| pairCompatibleAgent | Local models | **TARGET** | OPERATOR_CAS | Qualified agent/Runtime offer | |
| checkHardwareReadiness | Local models | **TARGET** | SCOPED_READ/OBSERVATION | Qualified agent/Runtime offer | |
| createWorkerProfile | Subagents | **TARGET** | OPERATOR_CAS | DelegationProfileService | |
| cloneProfile | Subagents | **TARGET** | OPERATOR_CAS | DelegationProfileService | |
| setWorkerCapabilities | Subagents | **TARGET** | OPERATOR_CAS | DelegationProfileService | |
| setParallelLimit | Subagents | **TARGET** | OPERATOR_CAS | DelegationProfileService | |
| setBudgets | Subagents | **TARGET** | OPERATOR_CAS | DelegationProfileService | |
| changeRouting | Subagents | **TARGET** | OPERATOR_CAS | DelegationProfileService | |
| archiveProfile | Subagents | **TARGET** | OPERATOR_CAS | DelegationProfileService | |
| searchCatalog | Connections | **TARGET** | SCOPED_READ/OBSERVATION | LiteSPM + ConnectionService + Trust | |
| filterKind | Connections | **TARGET** | SCOPED_READ/OBSERVATION | LiteSPM + ConnectionService + Trust | |
| previewPackage | Connections | **TARGET** | SCOPED_READ/OBSERVATION | LiteSPM + ConnectionService + Trust | |
| installPackage | Connections | **TARGET** | EXTERNAL_EFFECT | LiteSPM + ConnectionService + Trust | |
| connectAccount | Connections | **TARGET** | EXTERNAL_EFFECT | LiteSPM + ConnectionService + Trust | |
| assignCoworker | Connections | **TARGET** | EXTERNAL_EFFECT | LiteSPM + ConnectionService + Trust | |
| repairConnection | Connections | **TARGET** | EXTERNAL_EFFECT | LiteSPM + ConnectionService + Trust | |
| addLocalServer | MCP | **TARGET** | EXTERNAL_EFFECT | LiteSPM/CapabilityFabric | |
| addRemoteServer | MCP | **TARGET** | EXTERNAL_EFFECT | LiteSPM/CapabilityFabric | |
| showTransport | MCP | **TARGET** | SCOPED_READ/OBSERVATION | LiteSPM/CapabilityFabric | |
| testServer | MCP | **TARGET** | QUALIFIED_PROCESS_OR_NETWORK_PROBE | LiteSPM/CapabilityFabric | |
| disconnectServer | MCP | **TARGET** | EXTERNAL_EFFECT | LiteSPM/CapabilityFabric | |
| setToolAllowlist | MCP | **TARGET** | EXTERNAL_EFFECT | LiteSPM/CapabilityFabric | |
| previewSkill | Skills | **TARGET** | SCOPED_READ/OBSERVATION | LiteSPM package owner | |
| inspectManifest | Skills | **TARGET** | SCOPED_READ/OBSERVATION | LiteSPM package owner | |
| testSkill | Skills | **TARGET** | QUALIFIED_ISOLATED_EXECUTION | LiteSPM package owner | |
| addSkill | Skills | **TARGET** | EXTERNAL_EFFECT | LiteSPM package owner | |
| updateSkill | Skills | **TARGET** | EXTERNAL_EFFECT | LiteSPM package owner | |
| removeSkill | Skills | **TARGET** | EXTERNAL_EFFECT | LiteSPM package owner | |
| discoverChrome | Browser | **TARGET** | SCOPED_READ/OBSERVATION | BrowserSessionService/Provider | |
| selectChromeChannel | Browser | **TARGET** | LOCAL_SELECTION_NO_CONNECTION | BrowserSessionService/Provider | |
| createManagedProfile | Browser | **NATIVE** | NATIVE_MUTATION | BrowserSessionService/Provider | |
| previewBookmarkImport | Browser | **TARGET** | SCOPED_READ/OBSERVATION | BrowserSessionService/Provider | |
| commitImport | Browser | **NATIVE** | NATIVE_MUTATION | BrowserSessionService/Provider | |
| connectExistingChrome | Browser | **NATIVE** | NATIVE_MUTATION | BrowserSessionService/Provider | |
| detachSession | Browser | **NATIVE** | NATIVE_MUTATION | BrowserSessionService/Provider | |
| setSiteAccess | Browser grants | **TARGET** | AUTHORITY_CHANGE | TrustService/BrowserLeaseArbiter | |
| setObservePermission | Browser grants | **TARGET** | AUTHORITY_CHANGE | TrustService/BrowserLeaseArbiter | |
| setDownloadPermission | Browser grants | **TARGET** | AUTHORITY_CHANGE | TrustService/BrowserLeaseArbiter | |
| resetChromeProfile | Browser grants | **TARGET** | AUTHORITY_CHANGE | TrustService/BrowserLeaseArbiter | |
| deleteManagedProfile | Browser grants | **TARGET** | AUTHORITY_CHANGE | TrustService/BrowserLeaseArbiter | |
| selectDisplayTarget | Computer | **TARGET** | AUTHORITY_CHANGE | EnvironmentProvider/TrustService | |
| setScreenObservationPolicy | Computer | **TARGET** | AUTHORITY_CHANGE | EnvironmentProvider/TrustService | |
| grantDesktopUse | Computer | **TARGET** | AUTHORITY_CHANGE | EnvironmentProvider/TrustService | |
| revokeDesktopUse | Computer | **TARGET** | AUTHORITY_CHANGE | EnvironmentProvider/TrustService | |
| calibrateInput | Computer | **TARGET** | AUTHORITY_CHANGE | EnvironmentProvider/TrustService | |
| createQuickCoworker | Coworker identity | **TARGET** | OPERATOR_CAS | CoworkerService/TaskService | |
| editNameAndRole | Coworker identity | **TARGET** | OPERATOR_CAS | CoworkerService/TaskService | |
| chooseChief | Coworker identity | **TARGET** | OPERATOR_CAS | CoworkerService/TaskService | |
| pauseNewWork | Coworker identity | **TARGET** | OPERATOR_CAS | CoworkerService/TaskService | |
| pauseAllSafely | Coworker identity | **TARGET** | OPERATOR_CAS | CoworkerService/TaskService | |
| resumeCoworker | Coworker identity | **TARGET** | OPERATOR_CAS | CoworkerService/TaskService | |
| archiveCoworker | Coworker identity | **TARGET** | OPERATOR_CAS | CoworkerService/TaskService | |
| setLearningMode | Memory | **TARGET** | OPERATOR_CAS | Learning/Context authority | |
| listMemory | Memory | **TARGET** | SCOPED_READ/OBSERVATION | Learning/Context authority | |
| inspectSource | Memory | **TARGET** | SCOPED_READ/OBSERVATION | Learning/Context authority | |
| correctMemory | Memory | **TARGET** | OPERATOR_CAS | Learning/Context authority | |
| resolveConflict | Memory | **TARGET** | OPERATOR_CAS | Learning/Context authority | |
| forgetSource | Memory | **TARGET** | OPERATOR_CAS | Learning/Context authority | |
| deleteMemory | Memory | **TARGET** | OPERATOR_CAS | Learning/Context authority | |
| attachSource | Knowledge | **TARGET** | OPERATOR_CAS | ResourceIndex/Context authority | |
| selectFolderOrCloudResource | Knowledge | **TARGET** | OPERATOR_CAS | ResourceIndex/Context authority | |
| previewIndex | Knowledge | **TARGET** | SCOPED_READ/OBSERVATION | ResourceIndex/Context authority | |
| pauseIndex | Knowledge | **TARGET** | OPERATOR_CAS | ResourceIndex/Context authority | |
| removeSource | Knowledge | **TARGET** | OPERATOR_CAS | ResourceIndex/Context authority | |
| chooseRetrievalScope | Knowledge | **TARGET** | OPERATOR_CAS | ResourceIndex/Context authority | |
| createGoal | Goals | **TARGET** | OPERATOR_CAS | GoalService | |
| updateGoal | Goals | **TARGET** | OPERATOR_CAS | GoalService | |
| relateTask | Goals | **TARGET** | OPERATOR_CAS | GoalService | |
| inspectEvidence | Goals | **TARGET** | SCOPED_READ/OBSERVATION | GoalService | |
| archiveGoal | Goals | **TARGET** | OPERATOR_CAS | GoalService | |
| createDraftRoutine | Routines | **TARGET** | OPERATOR_CAS | RoutineService | |
| editRoutine | Routines | **TARGET** | OPERATOR_CAS | RoutineService | |
| saveRevision | Routines | **TARGET** | OPERATOR_CAS | RoutineService | |
| runOnce | Routines | **TARGET** | OPERATOR_CAS | RoutineService | |
| inspectRuns | Routines | **TARGET** | SCOPED_READ/OBSERVATION | RoutineService | |
| archiveRoutine | Routines | **TARGET** | OPERATOR_CAS | RoutineService | |
| createAutomation | Automations | **TARGET** | OPERATOR_CAS | AutomationService/TriggerCoordinator | |
| previewSchedule | Automations | **TARGET** | SCOPED_READ/OBSERVATION | AutomationService/TriggerCoordinator | |
| setTrigger | Automations | **TARGET** | OPERATOR_CAS | AutomationService/TriggerCoordinator | |
| setTimezone | Automations | **TARGET** | OPERATOR_CAS | AutomationService/TriggerCoordinator | |
| chooseOverlapPolicy | Automations | **TARGET** | OPERATOR_CAS | AutomationService/TriggerCoordinator | |
| reviewGrants | Automations | **TARGET** | OPERATOR_CAS | AutomationService/TriggerCoordinator | |
| enable | Automations | **TARGET** | OPERATOR_CAS | AutomationService/TriggerCoordinator | |
| pause | Automations | **TARGET** | OPERATOR_CAS | AutomationService/TriggerCoordinator | |
| runNow | Automations | **TARGET** | OPERATOR_CAS | AutomationService/TriggerCoordinator | |
| archive | Automations | **TARGET** | OPERATOR_CAS | AutomationService/TriggerCoordinator | |
| setQuietHours | Notifications | **TARGET** | OPERATOR_CAS | NotificationService/NeedsYou | |
| muteClass | Notifications | **TARGET** | OPERATOR_CAS | NotificationService/NeedsYou | |
| setDeliveryChannel | Notifications | **TARGET** | OPERATOR_CAS | NotificationService/NeedsYou | |
| viewQueue | Notifications | **TARGET** | SCOPED_READ/OBSERVATION | NotificationService/NeedsYou | |
| openRequest | Notifications | **TARGET** | LOCAL_AUTHORIZED_NAVIGATION | NotificationService/NeedsYou | |
| acknowledge | Notifications | **TARGET** | OPERATOR_CAS | NotificationService/NeedsYou | |
| inspectHealth | Runtime | **TARGET** | SCOPED_READ/OBSERVATION | RuntimeLifecycleService | |
| startDaemon | Runtime | **NATIVE** | NATIVE_MUTATION | RuntimeLifecycleService | |
| setStartupMode | Runtime | **NATIVE** | NATIVE_MUTATION | RuntimeLifecycleService | |
| stopSafely | Runtime | **NATIVE** | NATIVE_MUTATION | RuntimeLifecycleService | |
| inspectRecovery | Runtime | **TARGET** | SCOPED_READ/OBSERVATION | RuntimeLifecycleService | |
| viewActiveEnvironments | Runtime | **TARGET** | SCOPED_READ/OBSERVATION | RuntimeLifecycleService | |
| viewActiveGrants | Security | **TARGET** | SCOPED_READ/OBSERVATION | TrustService/OS SecretStore | |
| revokeGrant | Security | **TARGET** | AUTHORITY_CHANGE | TrustService/OS SecretStore | |
| inspectCredentialPresence | Security | **TARGET** | SCOPED_READ/OBSERVATION | TrustService/OS SecretStore | |
| rotateLocalSecret | Security | **TARGET** | AUTHORITY_CHANGE | TrustService/OS SecretStore | |
| viewEgressScope | Security | **TARGET** | SCOPED_READ/OBSERVATION | TrustService/OS SecretStore | |
| setRetentionPolicy | Security | **TARGET** | AUTHORITY_CHANGE | TrustService/OS SecretStore | |
| previewBackup | Backup | **TARGET** | SCOPED_READ/OBSERVATION | StorageBackup/Restore service | |
| exportWorkspace | Backup | **NATIVE** | NATIVE_MUTATION | StorageBackup/Restore service | |
| verifyManifest | Backup | **TARGET** | SCOPED_READ/OBSERVATION | StorageBackup/Restore service | |
| importBackup | Backup | **NATIVE** | NATIVE_MUTATION | StorageBackup/Restore service | |
| previewRestore | Backup | **TARGET** | SCOPED_READ/OBSERVATION | StorageBackup/Restore service | |
| confirmRestore | Backup | **NATIVE** | NATIVE_MUTATION | StorageBackup/Restore service | |
| inspectStorageUsage | Storage | **TARGET** | SCOPED_READ/OBSERVATION | Storage/ArtifactRetention service | |
| previewSafeCleanup | Storage | **TARGET** | SCOPED_READ/OBSERVATION | Storage/ArtifactRetention service | |
| pruneAgentBuildDebris | Storage | **NATIVE** | NATIVE_MUTATION | Storage/ArtifactRetention service | |
| manageArtifactRetention | Storage | **NATIVE** | NATIVE_MUTATION | Storage/ArtifactRetention service | |
| inspectUsageTotals | Usage & Monitoring | **TARGET** | SCOPED_READ/OBSERVATION | UsageProjection/Telemetry/HealthObserver | |
| filterUsageDateRange | Usage & Monitoring | **TARGET** | LOCAL_FILTER_NO_AUTHORITY | UsageProjection/Telemetry/HealthObserver | |
| filterUsageAgent | Usage & Monitoring | **TARGET** | LOCAL_FILTER_NO_AUTHORITY | UsageProjection/Telemetry/HealthObserver | |
| inspectUsageConfidence | Usage & Monitoring | **TARGET** | SCOPED_READ/OBSERVATION | UsageProjection/Telemetry/HealthObserver | |
| inspectCostPriceSource | Usage & Monitoring | **TARGET** | SCOPED_READ/OBSERVATION | UsageProjection/Telemetry/HealthObserver | |
| inspectQuotaHealth | Usage & Monitoring | **TARGET** | SCOPED_READ/OBSERVATION | UsageProjection/Telemetry/HealthObserver | |
| inspectFailureTimeline | Usage & Monitoring | **TARGET** | SCOPED_READ/OBSERVATION | UsageProjection/Telemetry/HealthObserver | |
| exportUsageReport | Usage & Monitoring | **TARGET** | REVIEWED_LOCAL_EXPORT | UsageProjection/Telemetry/HealthObserver | |
| editWorkspaceDelegationSettings | Agents & Subagents | **TARGET** | OPERATOR_CAS | WorkspaceDelegationSettingsService | |
| setCoworkerDelegationPolicy | Coworkers & Subagents | **TARGET** | OPERATOR_CAS | CoworkerService | |
| inspectShellProfiles | Terminal | **TARGET** | SCOPED_READ/OBSERVATION | TerminalSessionService/TerminalProfileRegistry | |
| chooseDefaultShellProfile | Terminal | **TARGET** | LOCAL_PROFILE_PREFERENCE | TerminalSessionService/TerminalProfileRegistry | |
| previewCustomShellPath | Terminal | **TARGET** | SCOPED_READ/OBSERVATION | TerminalSessionService/TerminalProfileRegistry | |
| validateCustomShellPath | Terminal | **TARGET** | QUALIFIED_OS_PROBE | TerminalSessionService/TerminalProfileRegistry | |
| setTerminalScrollback | Terminal | **NATIVE** | NATIVE_MUTATION | TerminalSessionService/TerminalProfileRegistry | |
| setTerminalRetention | Terminal | **NATIVE** | NATIVE_MUTATION | TerminalSessionService/TerminalProfileRegistry | |
| setShellIntegration | Terminal | **NATIVE** | NATIVE_MUTATION | TerminalSessionService/TerminalProfileRegistry | |
| setTerminalPasteConfirmation | Terminal | **NATIVE** | NATIVE_MUTATION | TerminalSessionService/TerminalProfileRegistry | |
| viewTerminalHealth | Terminal | **TARGET** | SCOPED_READ/OBSERVATION | TerminalSessionService/TerminalProfileRegistry | |
| clearTerminalViewport | Terminal | **TARGET** | LOCAL_VIEWPORT_ONLY | TerminalSessionService/TerminalProfileRegistry | |
| showVersion | About/Diagnostics | **TARGET** | SCOPED_READ/OBSERVATION | Runtime diagnostics/Tauri bridge | |
| exportRedactedLogs | About/Diagnostics | **TARGET** | QUALIFIED_READ | Runtime diagnostics/Tauri bridge | |
| runProviderDiagnostic | About/Diagnostics | **TARGET** | QUALIFIED_PROVIDER_PROBE | Runtime diagnostics/Tauri bridge | |
| copyBuildInfo | About/Diagnostics | **TARGET** | SCOPED_READ/OBSERVATION | Runtime diagnostics/Tauri bridge | |
| openSupportDocs | About/Diagnostics | **TARGET** | SCOPED_READ/OBSERVATION | Runtime diagnostics/Tauri bridge | |

## Shell controls — row baseline (2026-11-01)

Source: [V1-UI-ACTION-STATE-LEDGER.md](V1-UI-ACTION-STATE-LEDGER.md). Status is **TARGET** until a served handler is mounted. Feature packages 001–011 own the behavior; they do not mark these rows SERVED.

| Surface | Control | Authority | Test IDs | Map status | operationId |
|---|---|---|---|---|---|
| Window/Startup | Launch/startup restore | RuntimeLifecycle/Workspace | Never load previous owner's protected content before scoped read | **TARGET** | |
| Titlebar | Workspace switcher | WorkspaceService | Increment selection epoch and reject stale responses/drafts | **TARGET** | |
| Titlebar | Global search/command palette | Resource/Conversation search owner | No slash execution; exact Workspace authorization | **TARGET** | |
| Titlebar | Quick Entry | ConversationService | Not a second Conversation store; preserve same draft | **TARGET** | |
| Titlebar | Notifications/NeedsYou count | NeedsYou | Never display zero when projection failed | **TARGET** | |
| Titlebar | Runtime health | RuntimeLifecycle | Serving daemon is not agent execution readiness | **TARGET** | |
| Left rail | New Session | ConversationService | No Coworker required, selected default binding | **TARGET** | |
| Left rail | Recent/history | ConversationService | No cross-Workspace leak; accessible virtualization | **TARGET** | |
| Left rail | Conversation rename/pin/archive/delete | ConversationService/owner | Do not cascade independent Tasks/Artifacts without impact approval | **TARGET** | |
| Left rail | Coworker roster/recent | Coworker/Conversation owner | Exact owner Conversation only; no unrelated fallback | **TARGET** | |
| Left rail | Create/edit/pause Coworker | CoworkerService | Active Task revision stays pinned; no forced connectors | **TARGET** | |
| Left rail | Work/Task filters | TaskService | Persisted canonical Task status, not fake progress | **TARGET** | |
| Left rail | Automations create/list | AutomationService | Draft is UI proposal state; canonical machine status separate | **TARGET** | |
| Left rail | Library/Resource finder | Resource/ArtifactService | Exact version and permission, source freshness | **TARGET** | |
| Left rail | Discover packages (MCP, Skills, Plugins) + Connections | LiteSPM/Capability | Human approval before host mutation; agent cannot approve itself; reconcile UNKNOWN before retry; install, Connection, auth, assignment, Grant, activation and invocation stay separate; no secrets | **TARGET** | |
| Left rail | Settings | Settings owners per ledger | Every one of 187 controls references per-control owner ledger | **TARGET** | |
| Settings | Workspace/Coworker Subagents policy | WorkspaceDelegationSettingsService/CoworkerService | Exact profile revisions and Workspace ceiling; Coworker can inherit or narrow; future Task admissions only; native model inheritance only when adapter enforces it | **TARGET** | |
| Main chat | Conversation header + agent picker | AgentControlDescriptor | Atomic dependent model/mode/effort refresh; preserve draft | **TARGET** | |
| Main chat | Message stream and tool calls | Conversation/Presentation | Semantic message authoritative; no double rendering | **TARGET** | |
| Main chat | Approve/answer provider request | ConversationService/Trust | Request version CAS and provider acceptance before resume | **TARGET** | |
| Composer | Slash and @ autocomplete | AgentModule qualified descriptor | Native meaning never executed by host UI; IME-safe | **TARGET** | |
| Composer | Attachments and context picker | ResourceService/Agent descriptor | Source pins and agent-specific support rechecked at send | **TARGET** | |
| Composer | Send/Stop/Retry | ConversationService/AgentTurnCoordinator | Idempotency and canonical Turn state; don't promise unsupported steer | **TARGET** | |
| Composer | Run as Task / Schedule / Save Routine | TaskService/Routine/AutomationService | No implicit standing approval or Task for plain chat | **TARGET** | |
| Right pane | Workbench open/close/resize/max | UI local preferences/Artifact owner | No background focus steal; close pane does not stop Task | **TARGET** | |
| Right pane | Workbench back/forward/tab history | Workbench/Trust | Revalidate object permission and immutable revision at reopen | **TARGET** | |
| Right pane | Browser profile/attach/import | BrowserSessionService | True Chrome/Chromium provider qualification; no secret import | **TARGET** | |
| Right pane | Browser tab/address/navigation | BrowserActionService/Trust | Site grant, tab generation, epoch and effect fence | **TARGET** | |
| Right pane | Take/Return control | BrowserLeaseArbiter | Input enabled only after epoch committed; stale actions fenced | **TARGET** | |
| Right pane | Computer target/OS permission | EnvironmentProvider/Trust | OS permission, active display, focus and DPI require real test | **TARGET** | |
| Right pane | Terminal surface | Qualified Terminal capability | Never raw webview shell execution; grant and sandbox enforced | **TARGET** | |
| Right pane | File/code/diff/media/office | Artifact/PresentationService | Exact versions, file trust, sandbox, unsaved edits | **TARGET** | |
| Right pane | MCP App / Interactive Artifact | MCP capability presentation / version-pinned ArtifactRuntimeSupervisor | Distinct containment owners; generated app Run/Stop is explicit, exact-version and OS-qualified; Save As creates a separate FILE Artifact only after host confirmation; app cannot mint authority or directly create Effects | **TARGET** | |
| Right pane | Inspector Task/Attempt/Effect | Task/EvidenceService | Provenance, Effect/verification separate from model reported text | **TARGET** | |
| Overlays | Modal/popover/keyboard Escape | Local UI + pending mutation owner | One priority modal, focus trap/return, Esc cannot cancel uncertain effect | **TARGET** | |
| Motion | Feedback transitions | Domain-driven reducer | 80/120/180/280ms only for observed progress, no fake cursor/ETA | **TARGET** | |
## Exit criteria (BT-0.4 done)

1. Every row in [V1-UI-SCREEN-CONTRACT.md](V1-UI-SCREEN-CONTRACT.md) shell table has a status here.  
2. All settings controls have a map status (LOCAL_ONLY, NATIVE, or TARGET). operationId or UNAVAILABLE reason is still required before any row becomes SERVED.  
3. No UI doc claims SERVED without matching OpenAPI + inventory evidence.  
4. [CURRENT-RUN.md](../CURRENT-RUN.md) may record the TARGET baseline. Full semantic alignment stays open until operationIds are evidenced.
