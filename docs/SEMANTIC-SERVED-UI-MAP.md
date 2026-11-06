# Semantic served-handler ↔ UI map

This map records which visible surface invokes which Tauri command, which Operator operation or local UI action receives it, what the owner persists or observes, and what the UI must say when the action is unavailable.

**Authority:** domain semantics remain in the owning contracts and machine schemas. Intended screens and states are in V1-UI-SCREEN-CONTRACT.md and V1-UI-ACTION-STATE-LEDGER.md. Current source/qualification evidence is in IMPLEMENTATION-STATUS.md. This map is traceability, not another API or state-machine authority.

## 1. Route inventory and interpretation

The machine inventory is docs/schemas/served-route-inventory.json, generated from build_operator_router and the exact Router builders it merges. The scanner follows same-file Router-returning helpers such as routes_with_state(); it excludes test-only routers and unmerged route builders.

| Surface | Count | Meaning |
|---|---:|---|
| OpenAPI operations registered in the composed production Operator router | 173 | Method/path is registered by the composed router |
| Authenticated local-only operation outside OpenAPI | 1 | POST /v1/stream; local Operator stream transport |
| OpenAPI operations not registered in the composed production router | 56 | Target contract only; do not call as live operations |

A registered route is not proof that a handler performs a consequential operation. Some routes are read-only projections; some intentionally fail closed; some persist a definition but do not dispatch a provider. Assess three separate layers:

- **Route:** registered or target-only according to the generated inventory.
- **Handler/service:** reads, persists a record, admits a mutation, returns unavailable, or delegates to a real provider.
- **UI:** control is enabled, disabled with reason, read-only, or local-only.

Do not convert route presence into “integrated” or “qualified”.

## 2. Conversation and composer

| User surface/control | UI implementation | Tauri command / bridge | Operator operation | Current behavior and UI rule |
|---|---|---|---|---|
| Recent Chats list / Load more chats | ConversationsPage + App sidebar | list_conversations | GET /v1/conversations — listConversations | Cursor-paginated Workspace list. Each page is validated, appended with conversation_id deduplication and fenced to the active Workspace/list epoch. Sidebar displays all loaded items and exposes Load more chats only while another cursor exists; loading and errors remain explicit |
| New Chat | ConversationsPage create flow | create_conversation / list_pending_conversation_creates | POST /v1/conversations — createConversation; local Tauri journal command | Persists Conversation with idempotency; pending-create journal restores ambiguous request identity. If the journal cannot be read, New Chat is paused and a visible Retry checking pending Conversation requests action re-reads it before further create requests are allowed |
| Open Conversation | ConversationsPage | get_conversation | GET /v1/conversations/{conversationId} — getConversation | Loads saved projection; never reuse data from prior Workspace |
| Agent picker/binding | ConversationsPage | list_agent_bindings / list_agent_profiles | GET /v1/agent-bindings — listAgentBindings; GET /v1/agent-profiles — listAgentProfiles | Profile discovery and binding availability do not prove a native turn adapter |
| Change selected agent | ConversationsPage | set_conversation_agent_binding | PATCH /v1/conversations/{conversationId}/agent-binding — setConversationAgentBinding | Persists selection for subsequent turns; does not transfer provider-private context |
| Send | ConversationsPage | submit_conversation_turn | POST /v1/conversations/{conversationId}/turns — submitConversationTurn | Persists USER message/OPEN turn/receipt, then detached-spawns the OpenCode-only supervisor. Send remains enabled despite the target disabled-until-qualified gate and does not require a qualified binding/provider or Operator readiness. No durable outbox/claim or startup sweep; submit does not enforce B-01 single-flight, so concurrent OPEN turns remain possible. The composer does not fence OPEN consistently |
| Composer affordances | ConversationsPage | No action owner for several controls | Add context/file, More options, Voice Input, Turbo and @/slash mention/action placeholder | Add context/file, More options and Voice Input have no handlers; Turbo is static; mention/action copy is placeholder. Submit sends resource_refs=[]; no attachment or Context Used flow is mounted |
| Active-turn gate | ConversationsPage | active_turn status projection | GET conversation snapshot | Projection exposes status without aggregate version. Composer considers RUNNING plus transient presentation turns active; OPEN/WAITING_DEPENDENCY/CANCEL_REQUESTED are not consistently fenced. WAITING_USER continuation semantics remain unresolved |
| Empty Conversation canvas | ConversationsPage | Static content | No backend owner | Hard-coded Antigravity branding and capability claims; not a source of product capability truth |
| Stop / cancel | ConversationsPage | cancel_conversation_turn | POST /v1/conversations/{conversationId}/turns/{turnId}/cancel — cancelConversationTurn | The visible Stop handler searches only presentation-stream transient turns; the OpenCode path does not emit them. If a transient exists, the UI hardcodes If-Match version 1. The Operator route transitions RUNNING→CANCEL_REQUESTED→CANCELLED immediately without native interrupt/quiescence; the visible Stop control is not effective for OpenCode |
| Retry | No active UI call site | retry_conversation_turn | POST /v1/conversations/{conversationId}/turns/{turnId}/retry — retryConversationTurn | Operator route exists, but no retry affordance/API call is wired. Backend moves FAILED to RUNNING with a new session pointer before dispatch; supervisor STARTING reservation requires OPEN/no session pointer, so retry is inconsistent |
| Saved transcript/presentation | ConversationsPage / PresentationRuntime | get_conversation_presentation / get_rich_presentation | GET presentation — getConversationPresentation; GET rich presentation — getRichPresentation | Committed semantic messages are authoritative; rich presentation is optional and digest-bound |
| Linked Tasks | Conversation surface / Task navigation | list_conversation_tasks | GET /v1/conversations/{conversationId}/tasks — listConversationTasks | Read projection only; normal chat does not implicitly create Tasks |
| Live stream | ConversationsPage / presentation stream | subscribe_conversation_stream / acknowledge_conversation_stream_frame / unsubscribe_conversation_stream | POST /v1/stream (local-only outside OpenAPI) | Rich-presentation/saved-history stream; not proof of a complete native token stream. The OpenCode dispatch path does not populate the transient-turn collection used by the Stop handler; reconnect must resync committed projections |

**Native adapter availability:** Agent registry, installation observations, profile probes, binding records and control descriptors are separate from a turn-execution adapter. The supervisor currently dispatches OpenCode; other provider selections must not be shown as Send-capable until their adapter/auth/OS route is qualified. A committed OPEN turn is not an answer.

## 3. Workspace, roots and Resources

| User surface/control | UI implementation | Tauri command | Operator operation | Current behavior and UI rule |
|---|---|---|---|---|
| Workspace list/select | App / Settings | list_workspaces and local selection epoch | GET /v1/workspaces — listWorkspaces | Persisted list; selection itself is local scope state and invalidates previous fetches |
| Create/update/archive Workspace | App / Settings | create_workspace / update_workspace_policy / archive_workspace | POST /v1/workspaces — createWorkspace; PATCH /v1/workspaces/{workspaceId} — updateWorkspacePolicy; POST /v1/workspaces/{workspaceId}/archive — archiveWorkspace | Real owner mutation path with version/request protection |
| Default AgentBinding | App / Settings | set_workspace_default_agent_binding | PATCH /v1/workspaces/{workspaceId}/default-agent-binding — setWorkspaceDefaultAgentBinding | Default for future turns; not a permission grant |
| Workspace runtime binding | App / Settings | list_workspace_runtime_bindings / enroll_local_runtime | GET runtime-bindings — listWorkspaceRuntimeBindings; GET current-local — getCurrentLocalRuntimeWorkspaceBinding; POST local-enrollment — enrollLocalRuntimeInWorkspace | Binding is Workspace authorization, not Runtime execution readiness |
| Workspace instructions | Settings | list_workspace_instructions / create_workspace_instruction_revision | GET instruction revisions — listWorkspaceInstructionRevisions; POST revision — createWorkspaceInstructionRevision | Immutable revisions; preserve draft on conflict |
| Workspace roots | App / Library Persistent folders / Workbench Files | list_workspace_roots / add_workspace_root / pause_workspace_root / resume_workspace_root / revoke_workspace_root / scan_workspace_root / search_workspace_root_observations | GET roots — listWorkspaceRoots; POST pause/resume/revoke/scan; GET observations — searchWorkspaceRootObservations; private local-only POST `/__litecowork_local/workspace-root-selection` | Add opens native picker and immediately commits on supported Linux/macOS; Windows handler returns 503 although Add remains enabled. No post-picker confirm; METADATA/NONE policies are hardcoded. Scan is Linux-only metadata census; Windows/macOS return 501 after first attempt, then UI disables further scans. No content ingestion/watch/index from root; observations are names/size/freshness only |
| Resource upload/import | App / Library | create_resource_upload / upload_resource_chunk / commit_resource_upload / import_resource | POST uploads — createResourceUpload; PUT chunk — uploadResourceChunk; POST commit — commitResourceUpload; POST quick-import — quickImportResource | Bounded upload/import path; Library accepts ZIP/PDF/Office/image originals as opaque Resources. ZIPIntakeNotice says ZIP contents are not unpacked/indexed/attached. Source ZIP metadata-preview route is dev-feature-only and has no UI call site |
| Resource list/detail/search | App / Library / Workbench Files | list_resources / get_resource_detail / search_resources | GET resources — listLocalResources; GET detail — getResource; GET search — searchResources | Persisted records; search result is not implicitly attached to a prompt |
| Resource revision/location/content | App / ResourceRevisionEditor / Workbench Files | list_resource_revisions / list_resource_locations / preview_resource_text / read_resource_content | GET revisions — listResourceRevisions; GET locations — listResourceLocations; GET content — readLocalResourceContent | Exact-revision read/preview; preview is not arbitrary file editing |
| Resource indexing | App / resource tools | rebuild_resource_text_index / get_resource_index_state | POST rebuild — rebuildResourceTextIndex; GET state — getResourceIndexState | Allowlisted UTF-8 text only, max 1 MiB; no PDF/Office/archive/media extraction. A rebuild request is not proof the index is current; show returned status/failure |
| Grounded retrieval | No current Conversation/Library action | No production UI call site | POST `/v1/resources/retrieval` — groundedRetrieval | Read-only local lexical retrieval with exact revision/digest citations or insufficient evidence; no durable receipt/event/ContextAttachment, no Context Used UI and not wired into Conversation send |
| ZIP readiness notice | Library / ZipIntakeNotice | get_zip_intake_readiness | GET readiness projection via Tauri bridge | Reports UNAVAILABLE / OPAQUE_RESOURCE_ONLY; ZIP originals remain stored intact but contents are not unpacked, indexed or attached |
| ContextDocument status/deletion | ResourceRevisionEditor | set_context_document_status / begin_context_document_deletion / get_context_document_deletion_status | PATCH status — setContextDocumentStatus; POST/GET deletion — requestContextDocumentDeletion/getContextDocumentDeletionStatus | Honor deletion state, tombstone and exact Workspace scope |

## 4. Work and durable Tasks

| User surface/control | UI implementation | Tauri command | Operator operation | Current behavior and UI rule |
|---|---|---|---|---|
| Work Task list/pagination | WorkPage / TaskList | list_tasks | GET /v1/tasks — listTasks | Persisted Task records; no synthetic progress |
| Create Task | Home composer / explicit Task creation | create_task | POST /v1/tasks — createTask | Persists Task specification; explicit creation only |
| Task detail/objective revision | TaskDetailPanel | get_task / revise_task_spec | GET task — getTask; POST spec revision — reviseTaskSpec | Persisted Task/spec revisions; not plan execution |
| Planning readiness | TaskDetailPanel | get_task_planning_readiness | GET planning-readiness — getTaskPlanningReadiness | Returns dispatch unavailable and no planner/session/plan start; UI states no work is running |
| Plan revisions/Steps | TaskDetailPanel projections | list_task_plan_revisions / list_task_steps | GET plan revisions — listPlanRevisions; GET Steps — listTaskSteps | Read surfaces are served; POST submitTaskPlan is target-only; do not imply an accepted plan exists |
| Attempts/Effects/invocations/timeline | Target Work Mission Control | No current UI command path | Target-only: listTaskAttempts, listTaskEffects, getTaskTimeline, listTaskUsage; invocation history GET is served but read-only | No running Attempt admission/execution vertical slice |
| Task cancel/pause/resume/steer/recover | Target Task controls | No current Tauri/UI action | Target-only: cancelTask, pauseTask, resumeTask, steerTask, recoverTaskStep | Disable/omit until owning transitions are served and qualified |
| Task Artifacts/verification | TaskPresentationPanel / outputs | get_task_presentation / get_task_progress / list_task_artifacts / list_task_verification_runs | GET Task presentation/progress/artifact/verification projections are served | Projection/history does not prove worker execution or verification success |

Mission Control belongs inside Work/Task detail, backed by actual Step, Attempt, Effect, Artifact and Evidence records. No percent complete, ETA or worker state may be invented.

## 5. Coworkers, delegation profiles and Goals

| User surface/control | UI implementation | Tauri command | Operator operation | Current behavior and UI rule |
|---|---|---|---|---|
| Coworker roster/create/detail | CoworkerSettings / App | coworker_request | GET/POST /v1/coworkers — listCoworkers/createCoworker; GET detail — getCoworker | Cursor-paginated roster, detail, create/edit and revision state; persisted identity/configuration, not a resident agent process |
| Coworker revisions/status | CoworkerSettings | coworker_request | GET revision — getCoworkerRevision; POST revisions — reviseCoworker; POST status — changeCoworkerStatus | Persists lead/delegation/profile allowlist/interaction/failover/context/notification settings. Only default lead is consumed by Task creation; other policies have missing production consumers |
| Coworker pause/admission | CoworkerSettings / AutomationEditor / Task composer | coworker_request / automation_request / create_task | changeCoworkerStatus; Automation enable transition; createTask | Automation enable checks Coworker active at enable time, but existing enabled Automation occurrence settlement does not re-check Coworker status; Task storage rejects ARCHIVED only. Task composer blocks PAUSED Coworkers even for owner Tasks. Pause copy overstates the actual fence |
| Home Quick Create | HomeCoworkerOnboarding | coworker_request / set_primary_coworker | createCoworker; setPrimaryCoworker | Creates a general Assistant and may set Workspace primary in separate commits; does not open a Coworker-owned Conversation |
| Task Coworker origin | Home Task composer | create_task | POST /v1/tasks — createTask | Pins Coworker ID/revision to Task provenance; Task origin is not Conversation ownership; composer currently accepts ACTIVE Coworkers only |
| Coworker presence | CoworkerSettings | coworker_request | GET presence — getCoworkerPresence | Returns 503 DEPENDENCY_UNAVAILABLE until committed Task/UserRequest/Attempt/Runtime projections exist; never fabricate “online” |
| Primary Coworker | Settings / onboarding | set_primary_coworker | POST /v1/workspaces/{workspaceId}/primary-coworker — setPrimaryCoworker | Workspace primary selection; separate from Conversation creation and authorization |
| Delegation profiles | Agent Settings / Coworker Settings | delegation profile bridge | GET/POST profile, revision, duplicate, status and performance routes; quota observation | Workspace profile CRUD/history/performance and per-binding quota are visible. Enabled means eligible for policy selection, not a running child worker; SQLite admission guard has no production daemon caller |
| Goals | GoalsPage | goal_request | GET/POST/revisions/status — listGoals/createGoal/reviseGoal/changeGoalStatus | Persisted passive Goal records; a Goal does not execute itself |
| Coworker memory/responsibilities | Target Coworker detail | No complete UI/service workflow | Owner target in LEARNING.md, CONTEXT.md, RESPONSIBILITIES.md | No Coworker-owned Conversation, private memory browser, Learning Inbox or scoped responsibility execution |

## 6. Automations and Routines

| User surface/control | UI implementation | Tauri command | Operator operation | Current behavior and UI rule |
|---|---|---|---|---|
| Routine list/create/detail/revisions | RoutinesPage / AutomationEditor | routine_request | GET/POST routines — listRoutines/createRoutine; GET detail/health/revisions — getRoutine/getRoutineHealth/listRoutineRevisions; POST revisions — reviseRoutine | Persisted reusable definitions and revisions |
| Routine run/archive | RoutinesPage | routine_request | POST run/archive — runRoutine/archiveRoutine | A routine run can materialize a Task; does not imply worker execution |
| Automation list/create/detail/revisions/health/occurrences | AutomationsPage | automation_request | GET/POST automations — listAutomations/createAutomation; GET detail/revisions/health/occurrences — getAutomation/listAutomationRevisions/getAutomationHealth/listAutomationOccurrences | Definition list is cursor-paginated with Load more. Detail loads current immutable Automation/Routine pins and health. Occurrence history reads only the first page and drops next_cursor; no older-history control |
| Automation edit/pause/resume/disable | AutomationEditor | automation_request | PATCH update; POST pause/resume/disable — updateAutomation/pauseAutomation/resumeAutomation/disableAutomation | Owner mutations are served |
| Run once/test run | AutomationEditor | automation_request | POST run/test-runs — runAutomationNow/runAutomationTest | Run once requires exactly one Manual trigger and commits an occurrence plus READY Task; Create test Task commits READY Task without occurrence/cursor. Neither starts planning/agent execution. UI unresolved request registry is process-local |
| Local TriggerHost | Daemon worker | Started by OperatorServer | POST /v1/automations/host/settle — settleAutomationTriggers | OperatorServer starts AutomationTriggerHostWorker; polls persisted cursors every 30 seconds; supported schedules only; requires awake/available Runtime; scheduler/materializer only. automation_operator.rs header claiming host and /resume unavailable is stale |

## 7. Discover, skills, proposals and LiteSPM

| User surface/control | UI implementation | Tauri command | Operator operation | Current behavior and UI rule |
|---|---|---|---|---|
| Explore search | DiscoverPage | discover_request | GET /v1/discover/search — searchDiscover | Workspace-local skill/catalog projections only |
| Item details | DiscoverPage review disclosure | discover_request | GET /v1/discover/items/{id} — describeDiscoverItem | Review-only; does not grant or install |
| Installed/native hosts | DiscoverPage | discover_request | GET runtime capability-hosts — listRuntimeCapabilityHosts | Bounded Runtime observation; empty observation is not proof there are no native installs |
| Skill list/detail/create/revise | Discover / skill UI | skill API/bridge | GET/POST skills — listSkills/createSkill; GET item — getSkill; PUT item — reviseSkill | Workspace skill metadata; not LiteSPM package inventory |
| Skill proposals | SkillProposalsPage | skill proposal API | GET/POST proposal/review — listSkillProposals/createSkillProposal/approveSkillProposal/rejectSkillProposal | Proposal lifecycle served; approval is not package installation |
| Capability Connect/Install/Test | Discover row | discover_request | POST connect-or-install/test — connectOrInstallCapability/testCapabilityConnection | Handler deliberately fails closed while LiteSPM service is unqualified; requires idempotency key but makes no package mutation/receipt. Keep disabled/unavailable |
| Managed packages/Connections | Discover tabs | No qualified package-service bridge | No qualified package inventory/Connection operations | UI explicitly reports unavailable; infer neither managed state nor account authentication |

LiteSPM owns catalogue, install/adopt/update/repair/unmanage/remove lifecycle and package configuration mutation. LiteCowork owns Workspace/Coworker assignment, grants and mediated invocation. Do not build a second package writer.

## 8. Needs You, approvals, requests, notifications and suggestions

| User surface/control | UI implementation | Tauri command | Operator operation | Current behavior and UI rule |
|---|---|---|---|---|
| Needs You list | TaskAttentionPage | list_needs_you | GET /v1/needs-you — listNeedsYou | Persisted projection over open/resolved items; failure must not render an empty inbox |
| Approval detail/resolve | ApprovalDetail | get_approval_detail / resolve_approval | GET approvals/list/detail — listApprovals/getApproval; POST resolve — resolveApproval | Durable owner decision with If-Match/idempotency; APPROVE validates the exact action digest but does not create a reusable grant, dispatch an Invocation or execute an Effect; no optimistic final decision |
| UserRequest detail/respond/dismiss | UserRequestDetail | get/respond/dismiss_user_request | GET list/detail — listUserRequests/getUserRequest; POST respond/dismiss — respondToUserRequest/dismissUserRequest | Persisted response/dismissal with exact request version |
| Notifications/preferences | NotificationsPage | notification_request | GET notifications/preferences — listNotifications/listNotificationPreferences; PUT preference — setNotificationPreference | Cursor-paginated read-only delivery history and versioned preference writes. Outcome producer creates PENDING only; no delivery sender/status consumer. preferred_channels is stored but ignored by producer; ON_CONDITION maps only to DEPENDENCY_BLOCKED; quiet-hours are display/preserve-only |
| Suggestions | SuggestionsPage | suggestion_request/action commands | GET list/preferences — listSuggestions/listSuggestionPreferences; POST actions — resolveSuggestion/snoozeSuggestion/acceptSuggestionTask; PUT preference — setSuggestionPreference | Proposals only; accepting may create Task under owner rules; no standing grant |

## 9. Library and Artifacts

| User surface/control | UI implementation | Tauri command | Operator operation | Current behavior and UI rule |
|---|---|---|---|---|
| Artifact/Library list | ArtifactLibrary | artifact_library_command | GET /v1/artifacts — listArtifacts; GET /v1/library — listLibrary | Saved, Task outputs (TRANSIENT) and Archived projections; no create/generate/import-convert action or served Artifact create route |
| Artifact detail/version/edit head/content | ArtifactWorkbench | artifact_read / artifact_edit_head / artifact_append_text_version | GET artifact/version/edit-head/content; POST text-version — getArtifact/getArtifactVersion/getArtifactTextEditHead/downloadArtifactVersion/appendArtifactTextVersion | Exact immutable versions and provenance; edit/append only managed text/plain up to 1 MiB. Markdown/CSV/JSON/XML/YAML preview is not editable round-trip; no professional format engine |
| Archive/promote/task output | ArtifactWorkbench / Task outputs | artifact actions | POST archive/promote; GET Task artifacts — archiveArtifact/promoteArtifact/listTaskArtifacts | Promote/archive are metadata-only. Task outputs are read-only and have no live Feature 005 producer; Office uploads remain Resources, not Artifacts |
| Compare / Save As | TextVersionDiff / ArtifactWorkbench / native Tauri bridge | getArtifactVersion/downloadArtifactVersion / native Save As | Exact stored-version comparison; exact managed-blob local save up to 10 MiB | Compare uses exact versions; Save As creates no Artifact/Resource revision, domain event or durable ExportReceipt. It is a local file save, not provider export |

## 10. Workbench tool truth

| Workbench surface | Current source behavior | Required UI truth |
|---|---|---|
| Start | Launcher | Provider readiness is an observation; a tile is not proof a service exists |
| Files | Resource projection and exact revision text preview | Not a general live filesystem editor |
| Browser | iframe/WebView preview | Not Chrome/CDP automation, profile attach, cookies or an agent browser sandbox |
| Computer | No qualified provider | Unavailable; no screen control |
| Terminal | Simulated output (React-local sample); no process spawn or IPC; arbitrary commands print fabricated exit code 0; UI claims pwsh/ConPTY and Job Object containment without process receipt | Must remove fabricated success and unsupported shell/containment badges; never imply execution |
| Diffs | No selected source file; preview lines are labeled by a fixed modulo pattern or a static sample patch is shown; summary is hard-coded +4/-1 | Not a VCS, ResourceRevision or Artifact diff; do not claim actual source changes or fabricated counts |
| Activity | `WorkbenchTab` type member only; no navigation control or render branch; final branch falls through to Diffs | No Activity surface is currently rendered; do not imply it exists until committed event/Attempt/process projections are wired |
| Inspector | Available detail projections | Unknown/unavailable stays distinct from process/effect state |

## 11. Settings and local UI-only state

The active App.tsx Settings branch mounts SettingsView.tsx. It exposes exactly four visible categories: General, Workspaces, Agents and Runtime. The exported SettingsPage function in App.tsx is unmounted and is not current UI evidence. The 16-category V1 Settings IA and 187 named controls across 30 groups remain target design only.

| Visible category/control | Active UI implementation | Backend path | Current classification |
|---|---|---|---|
| General: theme, density, text scale, reduced motion | Component-local useState values; no persistence or application to the wider UI | None | INERT COMPONENT STATE; not an applied or durable preference |
| General: Ctrl+N, Ctrl+W, Ctrl+K shortcut labels | Static labels only; no matching Ctrl/Meta key handler found in active UI source | None | DISPLAY-ONLY; shortcut behavior is not implemented |
| Workspaces: select/refresh/create/archive | SettingsView props into App.tsx | Workspace list/create/archive routes | SERVED for exact listed operations |
| Workspace details | Read-only ID/status/replication-policy/version projection | Workspace read projection | READ-ONLY |
| Workspace instructions | Edit draft and save immutable instruction revision | Workspace instruction revision routes | SERVED |
| Workspace roots, rename, storage-policy mutation, Runtime enrollment | Not rendered by mounted SettingsView; related callbacks/commands elsewhere do not create visible controls | Separate WorkspaceRoot, policy and Runtime-binding operations exist | NOT A SETTINGS CONTROL in the active UI |
| Agents: registry/inventory/profile probes/binding create-enable-disable/default lead | AgentCatalogSettings and desktop agent APIs | Registry/profile/binding operations | PARTIAL SERVED; not general install/auth/turn support |
| Agents: model/effort picker | Not qualified in current descriptor/dispatcher path | Feature 002 target | TARGET / UNAVAILABLE |
| Runtime: start/refresh/status/lifecycle/blockers | SettingsView props into App.tsx | Runtime status/start/lifecycle routes | SERVED for exact start/read operations; not Task execution readiness |
| Runtime: stop/drain/startup-policy mutation/enrollment | Not rendered by mounted SettingsView | No active control in this component | NOT A SETTINGS CONTROL |

The mounted Settings UI contains only General, Workspaces, Agents and Runtime. Browser Control/cache/TLS, Shell Engine, command-approval, Credential Store and Coworker Defaults are not rendered by the active Settings source. Workbench Browser/Terminal surfaces are owned by Feature 008 and must not be folded into Settings.

The Runtime card's “Connected (Localhost)” copy is inaccurate: the authenticated transport is an OS-local Unix socket or Windows named pipe, not loopback TCP. The card's “Operator API is serving and ready” copy must not be interpreted as Task execution readiness. The mounted component also does not render its Runtime error/loading props.

The 187-control ledger remains a target inventory, not shipped coverage. See [SETTINGS-AND-SHELL-SURFACE-AUDIT.md](SETTINGS-AND-SHELL-SURFACE-AUDIT.md) for the control-level source crosswalk and the unmounted alternate component note.

## 12. Machine inventory and validation

- Route inventory: schemas/served-route-inventory.json.
- Generator/validator: scripts/audit_served_routes.py.
- Screen/interaction target: V1-UI-SCREEN-CONTRACT.md.
- State/action target: V1-UI-ACTION-STATE-LEDGER.md.
- Settings target: V1-SETTINGS-CONTRACT-LEDGER.md.
- Current source behavior: IMPLEMENTATION-STATUS.md.
- Package ownership: V1-FEATURE-SDD-REGISTER.md and ../specs/README.md.

A UI row is complete only when it names the exact user action, UI control, Tauri command, composed route/native action, handler behavior, owner state/receipt, denial/failure/ambiguous/restart behavior, and acceptance evidence for the exact OS/provider/build.
