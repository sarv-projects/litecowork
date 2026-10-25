# V1 UI Action, States and Owner Trace Ledger

Status: normative target companion to EXPERIENCE, DESIGN-SYSTEM, MOTION, DESKTOP-SHELL-LLD, NATIVE-COMPOSER-LLD and RENDERING-CONFORMANCE-LLD. UI events are local reducer semantics, not new durable domain event names. A documented action is not served until its authentic Operator/native handler exists.

## Universal UI state contract

Each control must supply pointer/keyboard/screenreader interactions, focus-visible, accessible name, disabled-with-reason/repair, loading/empty/error/permission/offline/stale states, actual backend owner, optimistic conflict and accessible annunciation. User-entered drafts, selection and scroll anchors are preserved against async races. On Workspace switch, invalidate stale fetches and option descriptors. All actual mutations carry canonical owner version/idempotency and actor from authenticated transport.

## Controls by surface

| Surface | User control | Proposed reducer action | Owning authority | UI states | Transition and safety condition | Test IDs |
|---|---|---|---|---|---|---|
| Window/Startup | Launch/startup restore | RuntimeObserved + WorkspaceSelectionRestored | RuntimeLifecycle/Workspace | LOADING, READY, DEGRADED, NO_WORKSPACE, OFFLINE | Never load previous owner's protected content before scoped read | UIV1-001–003 |
| Titlebar | Workspace switcher | WorkspaceSelectionRequested -> ScopedSelectionCommitted | WorkspaceService | READY, PENDING, DIRTY_GUARD, DENIED | Increment selection epoch and reject stale responses/drafts | UIV1-002–003 |
| Titlebar | Global search/command palette | PaletteRequested -> Search/RouteRequested | Resource/Conversation search owner | IDLE, LOADING, EMPTY, ERROR, DENIED | No slash execution; exact Workspace authorization | UIV1-004–005 |
| Titlebar | Quick Entry | QuickEntryOpened -> DraftSaved/TurnRequested | ConversationService | EMPTY, NEEDS_AGENT, ELIGIBLE, PENDING | Not a second Conversation store; preserve same draft | UIV1-001,UIV1-011–020 |
| Titlebar | Notifications/NeedsYou count | ProjectionRefreshed/AttentionOpened | NeedsYou | KNOWN, UNKNOWN, STALE, OFFLINE | Never display zero when projection failed | UIV1-140–147 |
| Titlebar | Runtime health | ReadinessProbe / RepairOpened | RuntimeLifecycle | SERVING, QUALIFIED, DEGRADED, UNAVAILABLE | Serving daemon is not agent execution readiness | UIV1-001,UIV1-148–158 |
| Left rail | New Session | ConversationCreateRequested | ConversationService | PENDING, OPEN, ERROR | No Coworker required, selected default binding | UIV1-011 |
| Left rail | Recent/history | CatalogLoaded/ItemOpened | ConversationService | LOADING, EMPTY, ERROR, STALE | No cross-Workspace leak; accessible virtualization | UIV1-011–015 |
| Left rail | Conversation rename/pin/archive/delete | VersionedCommandRequested/ReceiptConfirmed | ConversationService/owner | CONFIRM, PENDING, CONFLICT, REJECTED | Do not cascade independent Tasks/Artifacts without impact approval | UIV1-013–015 |
| Left rail | Coworker roster/recent | CoworkerSelected | Coworker/Conversation owner | READY, EMPTY, STALE, UNAVAILABLE | Exact owner Conversation only; no unrelated fallback | UIV1-044–048 |
| Left rail | Create/edit/pause Coworker | CoworkerMutationRequested | CoworkerService | DRAFT, PENDING, NEEDS_REVIEW, CONFLICT | Active Task revision stays pinned; no forced connectors | UIV1-044–048 |
| Left rail | Work/Task filters | TaskListFiltersChanged | TaskService | LOADING, EMPTY, BLOCKED, ERROR | Persisted canonical Task status, not fake progress | UIV1-091–109 |
| Left rail | Automations create/list | AutomationOpened/DraftRequested | AutomationService | DRAFT, ENABLED, PAUSED, UNAVAILABLE | Draft is UI proposal state; canonical machine status separate | UIV1-128–139 |
| Left rail | Library/Resource finder | ResourceSearchSubmitted | Resource/ArtifactService | INDEXING, EMPTY, NO_ACCESS, STALE | Exact version and permission, source freshness | UIV1-058–071 |
| Left rail | Discover packages (MCP, Skills, Plugins) + Connections | CatalogLoaded/InventoryLoaded/PlanPreviewRequested/ApprovalResponded/MutationReconciled | LiteSPM/Capability | LOADING, INSTALLABLE, DISCOVERY_ONLY, UNSUPPORTED_HOST, OBSERVED, MANAGED, ADOPTED, DRIFTED, PLAN_REVIEW, PENDING_APPROVAL, APPLYING, UNKNOWN, NEEDS_AUTH, OFFLINE, ERROR | Human approval before host mutation; agent cannot approve itself; reconcile UNKNOWN before retry; install, Connection, auth, assignment, Grant, activation and invocation stay separate; no secrets | UIV1-110–119 |
| Left rail | Settings | SettingsRouteOpened | Settings owners per ledger | READY, LOADING, OFFLINE, NO_ACCESS | Every one of 187 controls references per-control owner ledger | UIV1-004–009 |
| Settings | Workspace/Coworker Subagents policy | DelegationSettingsSaved/CoworkerPolicySaved | WorkspaceDelegationSettingsService/CoworkerService | DRAFT, PENDING, CONFLICT, UNSUPPORTED, STALE | Exact profile revisions and Workspace ceiling; Coworker can inherit or narrow; future Task admissions only; native model inheritance only when adapter enforces it | UIV1-101–109 |
| Main chat | Conversation header + agent picker | SelectedBindingChanged | AgentControlDescriptor | LOADING, READY, AUTH_REQUIRED, STALE | Atomic dependent model/mode/effort refresh; preserve draft | UIV1-023–027 |
| Main chat | Message stream and tool calls | TurnEventObserved -> SnapshotReconciled | Conversation/Presentation | STREAMING, COMMITTED, DEGRADED, ERROR | Semantic message authoritative; no double rendering | UIV1-049–057 |
| Main chat | Approve/answer provider request | UserRequestResponded | ConversationService/Trust | PENDING, CONFIRMED, REJECTED, AMBIGUOUS | Request version CAS and provider acceptance before resume | UIV1-140–147 |
| Composer | Slash and @ autocomplete | NativePaletteOpened/TokenCommitted | AgentModule qualified descriptor | OPEN, EMPTY, STALE, INCOMPATIBLE | Native meaning never executed by host UI; IME-safe | UIV1-028–036 |
| Composer | Attachments and context picker | AttachmentSelected/Validated | ResourceService/Agent descriptor | READY, UNSUPPORTED, STALE, DENIED | Source pins and agent-specific support rechecked at send | UIV1-016–036 |
| Composer | Send/Stop/Retry | TurnAdmission/Cancel/RetryRequested | ConversationService/AgentTurnCoordinator | ELIGIBLE, SENDING, WAITING, AMBIGUOUS, SETTLED | Idempotency and canonical Turn state; don't promise unsupported steer | UIV1-016–022 |
| Composer | Run as Task / Schedule / Save Routine | TypedHostActionRequested | TaskService/Routine/AutomationService | DRAFT, REVIEW, PENDING, BLOCKED | No implicit standing approval or Task for plain chat | UIV1-091–100,UIV1-128–139 |
| Right pane | Workbench open/close/resize/max | PaneOpenRequested/CloseGuard/ResizeCommitted | UI local preferences/Artifact owner | OPEN, CLOSED, DIRTY_GUARD, STALE | No background focus steal; close pane does not stop Task | UIV1-006,UIV1-058–064 |
| Right pane | Workbench back/forward/tab history | PaneHistoryRequested | Workbench/Trust | READY, REVOKED, MISSING, UNSAVED | Revalidate object permission and immutable revision at reopen | UIV1-058–064 |
| Right pane | Browser profile/attach/import | BrowserOpen/Consent/ImportRequested | BrowserSessionService | UNAVAILABLE, CONSENT, CONNECTED, DETACHED | True Chrome/Chromium provider qualification; no secret import | UIV1-072–081 |
| Right pane | Browser tab/address/navigation | BrowserActionRequested | BrowserActionService/Trust | READY, PENDING, UNKNOWN, BLOCKED | Site grant, tab generation, epoch and effect fence | UIV1-078–090 |
| Right pane | Take/Return control | ControlTransferRequested | BrowserLeaseArbiter | AGENT, TRANSFER_PENDING, USER | Input enabled only after epoch committed; stale actions fenced | UIV1-082–084 |
| Right pane | Computer target/OS permission | ComputerControlRequested | EnvironmentProvider/Trust | READY, LOCKED, DENIED, STALE | OS permission, active display, focus and DPI require real test | UIV1-086–089 |
| Right pane | Terminal surface | TerminalOpen/CommandRequested | Qualified Terminal capability | UNAVAILABLE, READY, EXECUTING, STOPPING | Never raw webview shell execution; grant and sandbox enforced | UIV1-062–063 |
| Right pane | File/code/diff/media/office | ArtifactVersionOpened | Artifact/PresentationService | LOADING, READY, FALLBACK, REVOKED, DIRTY | Exact versions, file trust, sandbox, unsaved edits | UIV1-058–071 |
| Right pane | MCP App / Interactive Artifact | SandboxedSurfaceOpened / ArtifactRuntimeStartRequested / ArtifactRuntimeStopRequested / SaveProposalReviewed | MCP capability presentation / version-pinned ArtifactRuntimeSupervisor | PREVIEW_ONLY, STARTING, RUNNING, STOPPING, STOPPED, FAILED, REVOKED, INTERRUPTED | Distinct containment owners; generated app Run/Stop is explicit, exact-version and OS-qualified; Save As creates a separate FILE Artifact only after host confirmation; app cannot mint authority or directly create Effects | UIV1-062 |
| Right pane | Inspector Task/Attempt/Effect | OwnerDetailRequested | Task/EvidenceService | LOADING, READY, REVOKED, UNKNOWN | Provenance, Effect/verification separate from model reported text | UIV1-091–109 |
| Overlays | Modal/popover/keyboard Escape | OverlayTransition/FocusRestored | Local UI + pending mutation owner | READY, OPEN, EXITING, DIRTY_GUARD | One priority modal, focus trap/return, Esc cannot cancel uncertain effect | UIV1-004–009 |
| Motion | Feedback transitions | PersistedObservationChanged | Domain-driven reducer | REDUCED_MOTION, NORMAL | 80/120/180/280ms only for observed progress, no fake cursor/ETA | UIV1-049–057 |

## Animation and rendering conformance timeline

- PENDING is derived from a real command/observation, never from a timer. A toast cannot report success until a committed receipt or qualified external effect observation.
- Stream reconnection reconciles immutable committed messages with transient frames by message/session ID; preserve scroll, draft/IME focus and accessibility reading order.
- Right pane opening uses state-derived 180ms transform where qualified; browser frames should not be animated as if they were DOM. Resize has keyboard operability and debounce local preference persistence.
- On reduced motion, transitions are instantaneous but focus and ARIA announcements are identical. Menu/disclosure first focus moves only after the logical open commit; close restores its authorized origin.
- Browser Take/Return uses explicit TRANSFER_PENDING until new epoch receipt. No phantom typing, fake progress bars, or false model reasoning indicators.

This ledger enumerates 35 UI interaction groups outside the 187 Settings control entries. SOURCE/OS/USER acceptance still required; no UI code was created by this document.
