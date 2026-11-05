# Digest docs-05 (flows, UX journeys, coworkers, automation, learning, artifacts/evidence)

Files: 19 read / 19 listed | Lines read: 6,660 | Bytes: 445 KB (plan-listed, all opened in full)

## Per-file notes

### docs/V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md (70 lines)
- Normative TARGET cross-owner crosswalk, not a state/API/event owner; canonical states stay in STATE-MACHINES/API/EVENTS/STORAGE/TASK-RUNTIME.
- Defines **FL01–FL25** (no hyphen). Note: **no `UC-*` ID exists anywhere in this bucket**; the ID families are `FL01–FL25`, `F-C01–F-C14`, `UIV1-001–UIV1-158`.
- Flow map: FL01 first run, FL02 Workspace switch, FL03 new Conversation, FL04 native send, FL05 agent change mid-draft, FL06 slash/@, FL07 chat→Task, FL08 dispatch Step, FL09 delegated worker, FL10 mutating capability, FL11 connector setup, FL12 managed browser, FL13 existing-Chrome attach, FL14 browser action, FL15 take/return control, FL16 Workbench/Artifact, FL17 rich stream, FL18 Routine draft, FL19 automation enable, FL20 automation trigger, FL21 Needs You, FL22 memory proposal, FL23 pause/stop/restart, FL24 Workspace archive, FL25 backup/restore.
- Each chain must name 9 proof elements: intent+Workspace Principal, grants/input pins, admission owner+expected version, persisted receipt/record/outbox transaction, provider op with lease+effect identity, observed/reconciled outcome, independent Evidence, authorized UI projection, bounded recovery. "UI success animation never substitutes for step 4/7."
- FL18: Save alone persists a RoutineRevision and creates "NO Task or occurrence or effect". FL03/FL07: no Task automatically. FL19: "Failure to bind host must not show enabled". FL20: DST fold/gap, duplicates, sleep → skipped/blocked with evidence.
- 24 race classes with forbidden behaviours (double Send, cancel-vs-completion race, lease expiry, timeout-after-dispatch, revocation mid-child, browser transfer, redirect, DST fold, two-window approval, archive vs pending cursor).
- Status: "All 25 flow chains and 24 race classes are design acceptance targets, not achieved results"; "first-party UI does not yet implement native Send and actual browser takeover". **0 QUALIFIED flows.**

### docs/V1-UI-ACTION-STATE-LEDGER.md (58 lines)
- Normative target companion to EXPERIENCE/DESIGN-SYSTEM/MOTION/DESKTOP-SHELL-LLD; "UI events are local reducer semantics, not new durable domain event names".
- Universal contract per control: pointer/keyboard/screenreader, focus-visible, accessible name, disabled-with-reason, loading/empty/error/permission/offline/stale, owner version+idempotency; drafts/scroll anchors preserved across async races.
- 35 UI interaction groups with reducer actions (`WorkspaceSelectionRequested`, `TurnAdmission`, `UserRequestResponded`, `NativePaletteOpened/TokenCommitted`, `ControlTransferRequested`, `ArtifactRuntimeStartRequested`, …) and test IDs `UIV1-001`–`UIV1-158`.
- Notable states: Needs You count "Never display zero when projection failed"; MCP App/Interactive Artifact states `PREVIEW_ONLY|STARTING|RUNNING|STOPPING|STOPPED|FAILED|REVOKED|INTERRUPTED`; Discover states include `DRIFTED`, `PLAN_REVIEW`, `UNKNOWN`.
- Motion: 80/120/180/280 ms only for observed progress; "PENDING is derived from a real command/observation, never from a timer"; no phantom typing/fake ETA.
- "This ledger enumerates 35 UI interaction groups outside the 187 Settings control entries. SOURCE/OS/USER acceptance still required; no UI code was created by this document."

### docs/V1-WORKFLOW-EVENT-LLD.md (116 lines)
- Status NORMATIVE TARGET, owners E03/E04/E05/E08/E09/E10; "cross-surface implementation map, NOT a new orchestration engine".
- Unified entries: New Chat→AgentBinding→ConversationTurn; "Run as Task"→TaskSpecRevision→PlanRevision→Steps; Coworker chat→CoworkerRevision-pinned Conversation; Saved Routine→immutable RoutineRevision (no execution); Enable Automation→Occurrence→Task; External event→dedup→Occurrence→Task; Task Continue→follow-up Conversation; Worker handoff→child Attempt.
- 28 durable function contracts (createConversation, admitConversationTurn, materializeTaskFromTurn, acceptPlanRevision, reserveStepAttempt, checkStopCriteria, reconcileExternalEffect, admitOccurrence, reconcileMissedTrigger, publishArtifact, verifyOutcome, updateNeedsYou, …).
- Explicit state corrections: `CREATED`/`TURN_RUNNING`/`COMMITTED` are **not** canonical ConversationTurn states; `Planning` is a UI projection of `RUNNING`, not a persisted `PLANNING` enum; Automation draft/review are interface stages, not persisted statuses.
- Idempotency: Idempotency-Key↔RequestId, If-Match, X-Workspace-ID; timeout reuses same key; "never create a fresh key to repeat an uncertain external effect".
- Event families are target-only (`conversation.turn.*`, `automation.occurrence.*`, `effect.*`, …); UI-only events are not domain events. Points to 25 flows/24 races in the FL ledger.

### docs/AUTOMATION.md (668 lines)
- Automation LLD: "defines when or why a pinned Routine should create ordinary Tasks. It is not a workflow engine".
- **Settlement boundary (2026-10-22):** only terminal Task status settles an occurrence — `COMPLETED`→`COMPLETED`, terminal `FAILED`→`FAILED`, `CANCELLED`→`SKIPPED`. All other states (`INCOMPLETE`, `PAUSED`, `WAITING_USER`, `BLOCKED`, `NEEDS_USER`, `VERIFYING`, `READY`, `RUNNING`, `PAUSE_REQUESTED`, `CANCEL_REQUESTED`) hold `STARTED` and the `max_concurrent_occurrences` slot. Producer `SqliteAutomationHostStore::settle_started_occurrences_from_task_outcomes` commits occurrence + `automation.occurrence.settled.v1` + one notification decision in one transaction. **Code-tier claim.**
- Still release-blocking: trigger-host cursor retirement on disable, real OS notification settlement (`notification.delivery.changed.v1`), owner USER evidence.
- 12 trigger kinds (`SCHEDULE, ONE_SHOT, WEBHOOK, CONNECTOR_EVENT, CHANNEL_EVENT, RESOURCE_EVENT, TASK_EVENT, RUNTIME_EVENT, PROCESS_EXIT, COMMAND_EXIT, CONDITION_WATCH, MANUAL`); placement `HUB|SPECIFIC_RUNTIME|AUTO`; ANY semantics, no implicit AND; stable `trigger_id` across revisions. V1 ships only schedule, one-shot, manual, webhook, connector-event; others → `TRIGGER_UNSUPPORTED`.
- `MisfirePolicy = SKIP | RUN_ONCE_WHEN_AVAILABLE | CATCH_UP_BOUNDED{max_occurrences}`; `ambiguous_local_time: EARLIER|LATER`; `nonexistent_local_time: SKIP|NEXT_VALID`.
- Occurrence lifecycle `PENDING→CLAIMED→STARTED→COMPLETED|FAILED`, `WAITING_DEPENDENCY→STARTED`, `→SKIPPED`; `version` +1 per transition, `claim_epoch` separate fence; `UNIQUE (automation_id, trigger_id, occurrence_key)`; key = SHA-256 over ASCII `LiteCowork/AutomationOccurrence/v2` + NUL + length-prefixed fields (3 vectors given).
- Endpoints: `POST /v1/automations/{id}/run` (If-Match, Idempotency-Key, one ManualTrigger; PENDING/v1→CLAIMED/v2 epoch1→STARTED/v3 + READY Task atomically, no Plan/Step/Attempt), `POST /v1/automations/{id}/resume` (PAUSED→ENABLED, atomic cursor registration, `DISABLED` terminal).
- Exact-revision test runs (`automation_test_run_id`) create a real Task, no occurrence, no cursor movement; allowed only when PAUSED/ENABLED.
- Hosting modes `LOCAL_POLL` (default), `LOCAL_PUSH_WHEN_REACHABLE`, `LITECOWORK_RELAY`, `USER_SELF_HOSTED_RELAY`; webhook HMAC-SHA256 over raw bytes, constant-time, 5-minute replay baseline (Slack), GitHub 30 s / Slack 3 s ack; `TRIGGER_UNSUPPORTED`, `OCCURRENCE_CONFLICT`, `STALE_CLAIM_EPOCH`, `MISFIRE_LIMIT_EXCEEDED`.
- UX: Coding / General cowork starter gallery (inert, versioned Routine templates); When/Do language; Run now ≠ enable; Run history → **Continue** → one `PRIMARY_FOLLOW_UP` Conversation with a trusted run-context card.

### docs/COWORKERS-TARGET.md (444 lines)
- "Accepted Coworker target design, reconciled 2026-10-24 … **Not an assertion of implemented functionality**"; supplements ARCHITECTURE, existing schemas stay authoritative.
- 11 frozen decisions: Coworkers optional persistent identities; separate Coworkers nav; last-active conversation opens; Quick Create vs Customize; no numeric cap on configured capabilities; native auth per agent (no global LLM router); Settings→Subagents separate; scoped memory with quiet automatic learning; "A Coworker is NOT a new planning/model runtime"; desktop-first with honest sleep/lock states.
- Ownership table: Coworker→CoworkerService/CoworkerRevision; Responsibility→proposed `StandingResponsibility`; Goal→GoalService; Routine/Automation/Work; Memory→ResourceService ContextDocuments + LearningService; Artifact; Needs You.
- `StandingResponsibility.status = DRAFT|ENABLED|PAUSED|COMPLETED|EXPIRED|ARCHIVED`, plus derived `HEALTHY|DEGRADED|BLOCKED|UNKNOWN`; no independent planner/lease; admission checks BOTH Coworker and Responsibility status.
- Proactive status `OFF|ACTIVE|PAUSED_NEW_WORK|NEEDS_ATTENTION|UNAVAILABLE|ARCHIVED`; `Pause new work` fences admissions, `Pause all safely` requests real Task pause and may stay `PAUSE_REQUESTED`.
- **F-C01–F-C14** journeys with pre/post conditions: F-C01 Quick Create (two recoverable commits, distinct idempotency keys), F-C02 Advanced, F-C03 add-on-demand, F-C04 recurring work, F-C05 proactive signal (NO_ACTION, no notification), F-C06 Needs You, F-C07 offline/restart (sleep 9AM→1PM, configured misfire), F-C08 crash during send (Effect AMBIGUOUS, never resend), F-C09 scope edit, F-C10 switch chief, F-C11 routine drift, F-C12 pause all, F-C13 agent says "done" but file missing, F-C14 notification delivery.
- Learning: default `AUTO_CONFIRMED_PRIVATE` in Quick Create, `REVIEW_ALL`/`OFF` available; `AUTO_CONFIRMED_AND_LOW_RISK_INFERRED_PRIVATE` only after informed selection; "There is NO AUTOMATIC_WITH_APPROVED_SHARING commit mode"; candidate states owned by LEARNING.md.
- Proposed events (`conversation.coworker.assigned.v1`, `coworker.capability_assignment.*`, `coworker.responsibility.*`) explicitly unpromoted; §13 autonomy presets Ask me / Keep an eye on things / Take care of approved routines; new Coworkers start in "Ask me"; `CoworkerHandoff` is a relationship/projection, not a transport.
- Contradiction inside the file: §1 says the right Workbench "is visible at launch in its compact Start state", §14 (2026-10-10) says it "is **not** always open by default" and new chats start at maximum conversational width.

### docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md (64 lines)
- Living register updated 2026-11-01, "not a claim that every paragraph of the archive was reread".
- Resolved/locked: Feature 001 B-01–B-10 (serialized turn lane `409 Conflict`, Windows MSVC + Claude Code/Codex, 3-phase handshake, `commit_conversation_agent_response` atomic transaction, best-effort streaming + REST resync, UserRequest continuation, sliding-window context reduction, dual-mode materialization, Job Object/cgroup 10-min idle TTL, tiered usage attribution). Feature 001 ACCEPTED/LOCKED.
- Evidence-vocabulary map: `TARGET_ONLY→TARGET`, `CODE_IMPLEMENTED→CODE`, `INTEGRATION_PROVEN→INTEGRATED`; "Do not invent a third scale".
- Still open: 002–011 (B-11–B-29) — agent matrix/descriptor TTL, ingestion formats, portable context bound, environment provider, golden provider, one professional format, browser vs computer-use, minimal memory record/Quick Create binding, first trigger/local notification, GA OS and which of 187 settings groups may be enabled.
- Corrections dated 2026-11-01 across specs 001–011 (e.g. 001 AC-018 STARTING reservation allowed; 256 KiB = UserRequest detail caps, package cap 1,024 messages/4 MiB; `CONVERSATION_CONTEXT_LIMIT_EXCEEDED` "is not in the 131-code registry"; 008 takeover keeps lease ACTIVE but control epoch n+1; 009 PAUSED still allows explicit owner Task, ARCHIVED cannot originate).
- Known machine-contract drift: OpenAPI `Connection.status` omits `REVOKED`; `runtime.availability.changed.v1` lacks `STARTING`/`RECOVERING`.
- Says "no operationId is marked SERVED … no active daemon to inventory"; 229 operationIds / 148 events / 131 codes / 28 migrations were aligned 2026-10-31.

### docs/COVERAGE-MATRIX.md (174 lines)
- Traceability ledger, explicitly non-overriding: per concern the first link is the normative owner, later links supporting; ADRs "never override the current authority".
- Local authority order (7 levels) ranks ARCHITECTURE→PRODUCT→domain→PROTOCOLS/API/schemas→FLOWS/FAILURE-RECOVERY/EXPERIENCE→SECURITY/OBSERVABILITY/TESTING/BENCHMARKS→ADRs. **Omits IMPLEMENTATION-MASTER-PLAN and the user-supplied Rev 9 spec** named in the repo instructions.
- Coworker row: "Optional chat-first Coworkers, standing responsibility grouping, quiet scoped memory … (non-operational until schema/API implementation)" → COWORKERS-TARGET, COWORKER-FLOWS, RESPONSIBILITIES, CONTEXT.
- `task.created` carries `origin_coworker_id`/`origin_coworker_revision` as optional both-or-neither; task creation no longer requires a Coworker.
- 2026-10-22 product set: V1-PRODUCT-RELEASE-CONTRACT (E13-S03), V1-UI-SCREEN-CONTRACT (E02-S01), LITESPM-LITECOWORK-INTEROP-LLD (E10-S01), archived Task-first Home non-normative; sqlite-v28.sql for delegation settings.
- Deferred decisions (explicit, not blanks): LiteSPM wire contract, UI component library, numeric SLOs, cloud vendor, Operator transport impl, Rust crate boundaries.

### docs/DESIGN-SYSTEM.md (532 lines)
- v1 visual/component contract; framework choice is implementation detail, anatomy/tokens/states/A11y are contracts.
- Tokens: 25 primitive colours; semantic light set; dark set exists in prose but `.theme-dark` is undefined in code. Status table maps each meaning to semantic tokens **plus a required non-color cue**; `REPORTED`/`OBSERVED`/`VERIFIED` use distinct labels, only VERIFIED gets the success/check treatment.
- Contrast: ≥6.1:1 light / 7.3:1 dark on declared pairs; WCAG 2.2 AA 4.5:1 body, 3:1 large/boundaries. Type scale 12/14/16/20/24/32; 4px spacing base; radii 8/12/16, pill badges.
- Component contract table (40px desktop / 44px touch; 2px focus ring + 2px offset; state priority disabled>loading>pressed>focus>hover>default) plus Coworker/delegation components: `CoworkerCard` (derived presence, no avatar status), `DelegationTree` ("2 workers helping"), `CostIndicator` (unit+source+confidence; unknown never zero), `QuotaIndicator` (stale→"Usage unavailable"), `GoalCard`, `SuggestionCard` (not styled as an Approval), `ControlOwnerBadge`.
- Layout: 240px expanded rail / 64px collapsed, content min 560px, Inspector 320px, Workbench 420–720px; 900–1279 single secondary panel; <900 drawer/detail routes; <600 single column, 44px targets; 200% zoom reflow.
- Explicit CODE status (2026-10-22): only 11 base tokens defined (`styles.css:15-25`); no dark theme, no `prefers-color-scheme`; success/warning/danger hard-coded in ~25 places; no type/space/radius/shadow tokens consumed; 133 font declarations below the 12px floor; disabled styling opacity-only in nine places; `aria-busy` missing on primary Save/Create/Archive.
- Rich composition: `--rich-readable-max: 48rem`, `--rich-wide-max: 72rem` (only the 48rem literal exists); registered model-safe set RichText…ArtifactCollection; host-bound blocks rendered only by trusted adapters.

### docs/DESKTOP-SHELL-LLD.md (275 lines)
- NORMATIVE TARGET, owners E02/E03/E08/E09/E10/E13; canonical screen contract is V1-UI-SCREEN-CONTRACT. "Default launch = New Chat or last eligible Conversation, not Task-first Home"; "No mandatory Coworker or Task".
- Anatomy: GlobalTitleBar(Workspace, Search, QuickEntry, Notifications, RuntimeStatus), LeftRail, MainPane, one RightWorkbench(Start, Browser, Computer, Terminal, Artifact/Office, Code/Diff, Image/PDF, MCP App, Interactive Artifact Runtime, Task details); "One right Workbench only; do not build another competing right 'Inspector' pane"; "No Cowork versus Code top-level split".
- `DesktopLayoutState` (workspaceId, route, left mode, right open/widthRatio/maximized/active/back/forward, overlays, density, theme, reducedMotion, textScale, focusReturnId, layoutEpoch); `PaneStatus = READY|EMPTY|LOADING|PARTIAL|OFFLINE|NO_ACCESS|REVOKED|ERROR|STALE`. Persist layout per Workspace; never persist grant/lease/authorization.
- Geometry: compact Workbench ≈24% clamped 300–400 CSS px, centre ≥520 CSS px; <900 px becomes full-surface route; 200/400% zoom must not clip actions.
- 15 reducer functions (`routeTo`, `selectCoworkerChat`, `openWorkbench(reason USER_EXPLICIT|RESTORED)`, `hydrateLayout`, `deriveSidebarCounts`→UNKNOWN not zero, `showToast` never before receipt, `useScopedRequest`).
- Settings IA: 30 groups / **187 named operations** → V1-SETTINGS-CONTRACT-LEDGER; non-settings controls → **35 UI interaction groups**; "SettingsMutation<T>" pseudocode showing preflight→scopedOperatorCommand→`lockPendingAndReconcileSameIdempotencyKey`/`preserveDraftAndShowCurrentVersion`.
- Keyboard/UX rows: Ctrl/Cmd+K global search (never executes slash), Ctrl/Cmd+Enter submit exactly once, Escape closes highest safe overlay but cannot discard a pending side effect, Workbench close does not stop the Task; "Pause all safely" fences new admission.
- Animation examples (80/120/180 ms, TRANSFER_PENDING until epoch committed, no fake confetti/cursor) and quality gates (1440/1280/1024/900/600/400 widths, RTL, 50 Coworkers, 100 connections).

### docs/HOST-GUIDANCE.md (138 lines)
- Boundary: optional presentation guidance + versioned built-in assets are **not** a CapabilityRef or LiteSPM package: zero grants, activations, secrets, network, filesystem, Effects, authority.
- `HostSkillRef{host_skill_id, uri, version, content_digest, purpose: RESPONSE_DESIGN, authority_class: GUIDANCE_ONLY}`; separate `HostSkillId` namespace; `SkillLoadTarget = HOST_SKILL{...} | CAPABILITY_SKILL{...}`; HOST_SKILL needs no grant/activation and cannot supply tool operations.
- `HostInstructionDeliveryMode = NATIVE_SYSTEM|NATIVE_DEVELOPER|SESSION_GUIDANCE|CONTEXT_ATTACHMENT|UNSUPPORTED`; UNSUPPORTED keeps the agent eligible. Never edits `CLAUDE.md`, `AGENTS.md`, Codex/OpenCode config, hooks, MCP servers or permission files.
- `SessionSpec.host_instruction_bundle_ref?`, `preloaded_host_skill_refs[]`; default CONVERSATION sessions only, not Task planning/worker Attempts.
- 7-point always-on policy (answer plainly first; rich only when materially better; never invent Artifact/Task/Approval/Verification state; preserve semantic fallback); "a hint, not security policy".
- Integrity failure → do not load, continue semantic response, sanitized local diagnostic.
- Learning boundary: agent may *propose* memory/instruction/Skill/Routine/Automation candidates via LearningService; a future commit is allowed only for a `LearningCandidate` already `AUTO_COMMIT_ELIGIBLE`, never arbitrary model text.

### docs/NETWORK-SECURITY.md (312 lines)
- Boundary: untrusted Agents/MCP/Apps/pages/Environments must not control Runtime authority; broker outside the sandbox; container firewall alone is insufficient.
- Egress deny-by-default with 7 rules: URL normalize; A/AAAA resolution incl. IPv4-mapped/scoped IPv6; reject loopback/private/link-local/multicast/reserved/ULA/cloud-metadata; pin address; re-resolve on retry/redirect and cap redirects; DNS-rebinding defence with original host for SNI; force broker + strip proxy vars/block QUIC; timeouts, header/body/decompression limits, MIME policy; strict TLS with no bypass.
- Local IPC: localhost TCP is not identity; Windows named pipe with current logon SID/SYSTEM DACL + remote-client rejection; Linux/macOS peer UID. "not yet fully cross-build-, system-, or OS-qualified"; "No loopback HTTP/bearer fallback is present".
- Capability Gateway token bound to AgentSession+Runtime, scope (`CONVERSATION|TASK_PLANNING|ATTEMPT_EXECUTION`), Workspace/Task/Attempt, method allowlist+grants, expiry/revocation epoch; rechecked on every call.
- Interactive Artifact renderer: no network at OS profile *and* CSP ("CSP alone is not an isolation boundary"), no Tauri bridge/token/port.
- UserRequest input: FORM non-sensitive; MCP `2026-07-28` `elicitation/create` URL mode keeps the URL in the encrypted local provider binding, HTTPS only, no userinfo/IP-literal hosts, `Cache-Control: no-store`, never logged/backed up; unknown methods incl. sampling unsupported.
- Fencing credentials: HMAC issuer key in OS keystore/HSM, durable record stores `fencing_token_digest` only; "a copied credential is not sufficient authority".
- Filesystem: WorkspaceRoots only, no-follow handle traversal, atomic replace; pseudonymous FileIdentity keyed by a Runtime key not in backup; startup identity mismatch → ACTIVE root becomes UNAVAILABLE, PAUSED stays PAUSED, "no path substitution". Windows root reopen has "partial CODE evidence only" and must fail closed.
- Archives/uploads: size/entry/depth/path limits, reject traversal/symlink/zip bombs; MCP Apps iframe without `allow-same-origin`, `default-src 'none'`, no invented fallback domains.
- OAuth loopback: ephemeral loopback listener, fresh state+PKCE, one terminal result, threat tests incl. port squatting, state replay, PKCE downgrade, commit-failure-after-exchange.

### docs/PROTOCOLS.md (187 lines)
- Five protocol boundaries: Operator (human clients), Mesh (trusted runtimes), Agent (Runtime→external agents), Capability (Gateway/Broker), Environment Provider.
- Operator scoping: every Workspace-scoped request carries `X-Workspace-ID` (IPC equivalent field); only Workspace list/create are unscoped; mismatch → `FORBIDDEN` "without revealing whether a referenced Workspace or resource exists"; subscriptions bound to the selected context; never exposes raw DB tables.
- Agent protocol selection by topology: ACP interactive, A2A remote systems, vendor SDK/API, qualified CLI fallback ("not a universal ranking").
- Host delegation surface `litecowork.agents.delegate(DelegateRequest)` → `ChildAttemptRef`; separate from native subagents. `litecowork.presentation.propose(RichPresentationIntent)` is bounded/ephemeral/zero-authority, not available to Task planners.
- Two delegation planes: durable control (admission/steer/cancel/settlement/Effect/Artifact/Evidence/lease) vs ephemeral streaming/observations that can drop and replay from checkpoint.
- Capability: MCP-compatible tools, stateless MCP base protocol dated `2026-07-28`; MCP Task handles persist on CapabilityInvocation and are "never confused with LiteCowork Task identity".
- `ProtocolHello{protocol_name, protocol_version, implementation_version, feature_flags[]}`; incompatible majors fail explicitly; not imposed on third-party MCP/ACP/A2A/LiteSPM.
- Three separate planes: AgentLifecycleAdapter (install/update/auth/descriptor) ≠ AgentAdapter session protocol ≠ LiteSPM package lifecycle.

### docs/RICH-RESPONSE.md (318 lines)
- `ConversationMessage` = semantic durable truth; `RichPresentation` = optional immutable enhancement. "No factual claim, warning, citation, deliverable, action, or system state may exist only in RichPresentation."
- `PresentationPreference = AUTO|SIMPLE|RICH`; `PresentationPolicyDecision = PLAIN|RICH_PREFERRED`; preference captured immutably on the turn and reused on retry.
- `RichPresentationIntent{schema_version:1, strategy, density, elements[], submitted_at_sequence}`; strategies `AUTO|PROSE|REPORT|COMPARISON|EXPLAINER|DASHBOARD|TIMELINE|DELIVERABLES|GALLERY`.
- Document bound by `semantic_content_digest` = SHA-256 over RFC 8785 JCS of `{content, resource_refs}`; `SemanticTextProjection` = TEXT blocks joined by one LF; `MessageTextSliceRef{start_utf8_byte, end_utf8_byte_exclusive, slice_digest}`; `block_provenance[]` keyed by unique RFC 6901 pointers; `BlockOrigin` 7 values, host-assigned only.
- 16 block rows = 14 model-safe (`TEXT_SLICE…ARTIFACT_COLLECTION`) + 2 host-bound (`HOST_PROJECTION`, `MCP_APP`), each with a deterministic fallback component; document ≤ max depth 6 and 512 KiB.
- **Stated non-implementation (2026-10-22):** `litecowork.presentation.propose` has "no implementation anywhere"; no citation resolver (any citation ref rejects the whole document); `DELIVERABLE_GROUP`/`ARTIFACT_COLLECTION` rejected by the desktop parser; `CHECKLIST` rejected with no store; `ArtifactBundleBuilder`, `ToolResultPresentationRegistry`, `PortableResponseRenderer` are target with no implementation.
- Rules: charts/tables use bounded `StructuredDataBinding` (one source: invocation result, pinned revision, or message table slice); NaN/Infinity rejected; diagrams Mermaid-only, sanitized, animation `NONE|DRAW_ONCE|HIGHLIGHT_SEQUENCE`; media only pinned Resource revisions, no model URLs, autoplay false.
- "Creating files is Task work. A Conversation AgentSession cannot publish Artifacts." Checklist selections are `LOCAL_TRANSIENT`/`LOCAL_DURABLE` UI state, never Task Steps or Verification. Search/export start from semantic content.

### docs/SCHEMAS.md (1,627 lines)
- Canonical language-neutral value types + shared enums + error taxonomy; entity fields in DATA-MODEL, transitions in STATE-MACHINES.
- Conventions: opaque IDs, RFC 3339 UTC (HLC for cross-Runtime ordering), integer ms, SHA-256 v1 digest, RFC 8785 JCS, "Missing authority … never assigned an implicit permissive default"; unknown command enum values rejected.
- `DeviceId = "ed25519-sha256:" + hex(SHA256(pubkey))`; private seed only in OS credential store, absent from SQLite/backups/events/logs.
- **Current-source narrowing (CODE):** `AgentInstallationState = MISSING|INSTALLED|VERSION_UNAVAILABLE`, `AgentAuthenticationReadiness = UNKNOWN`, `AgentSessionReadiness = NOT_PROBED`; the fuller `AgentInstallState`/`AgentAuthState` sets "have no producer in this build" and may not be reported.
- Status enums: `ConversationTurnStatus` 8 (`OPEN|RUNNING|WAITING_USER|WAITING_DEPENDENCY|COMPLETED|FAILED|CANCEL_REQUESTED|CANCELLED`); `TaskStatus` 13; `StepStatus` 11; `AttemptStatus` 11; `AgentSessionStatus` 6; `AutomationOccurrenceStatus` 7; `AutomationStatus = ENABLED|PAUSED|DISABLED`; `RoutineStatus = ACTIVE|ARCHIVED`; `EffectState` 8 ending `AMBIGUOUS`; `EvidenceLevel = REPORTED|OBSERVED|VERIFIED`; `ConnectionStatus` 5 (no `REVOKED`).
- `ConversationContextPackage`: same-Conversation committed messages only, ordered by message-added aggregate revision (not clock); excludes SYSTEM_NOTICE, drafts, native handles; **limit 1,024 messages / 4 MiB** → `CONVERSATION_CONTEXT_LIMIT_EXCEEDED` (note: the register says this code is absent from the 131-code registry, and it is indeed missing from the prose list below).
- Needs You: `item_id` = `sha256:` + SHA-256 of `litecowork.needs-you.item.v1` + NUL + length-prefixed fields (`APPROVAL,…`|`USER_REQUEST,…`|`TASK_BLOCKER,task_id,blocker_id`); blockers linked to an Approval/UserRequest collapse into one item; OPEN only while Task is `BLOCKED`/`NEEDS_USER`.
- Bounds: `UserRequestDetail.prompt ≤ 262144` bytes, choices ≤ 524288, encoded response ≤ 921600; `GET /v1/resources/{id}/content` `max_bytes 1..10_485_760`; `ResourceContentScanInfo` max_candidates 20 / 1_048_576 per file / 8_388_608 total; `EscalationPolicy.max_worker_attempts 1..8`; `LeadFailoverPolicy.max_lead_changes 0..3`; `DelegationBudgetPolicy` children 1..8, host depth 0..2; `ActionBatch` 1..64; 64-profile delegation settings arrays.
- ContextDocument lifecycle `ACTIVE|REVOKED|DELETION_PENDING|DELETED` with sealed immutable `ContextDocumentPurgePlan`; DELETED only when receipts == target count; tombstones preserve identity/digests/provenance.
- Reserved future identifiers (`ConnectorId, AuthAttemptId, LearningCandidateId, MemoryRecordId, FeedbackId`) and `LearningCandidateState = NEW|CLASSIFIED|NEEDS_REVIEW|AUTO_COMMIT_ELIGIBLE|COMMITTED|DISMISSED|REJECTED|SUPERSEDED`; `CoworkerContextPolicy.require_user_confirmation_for_memory` labelled a legacy implemented field, not the new learning policy.

### docs/SERVICES.md (1,568 lines)
- Fixes service ownership + dependency direction; every mutating command takes `CommandContext` (Idempotency-Key, X-Workspace-ID, principal, correlation); expected_version stays command-specific.
- WorkspaceService: v1 has exactly one owner principal; `archive` requires terminal Tasks, disabled Automations, settled turns/Invocations, no grants/SecretLeases/control leases, live-workload-free persistent Environments, else `WORKSPACE_NOT_QUIESCENT`; archived Workspace rejects every mutation but keeps reads/downloads.
- ConversationService: `submit_turn` resolves the binding first and returns `AGENT_UNAVAILABLE` before persisting any message/turn (draft preserved); adapter failure after commit settles the turn FAILED without erasing the user message; `retry_turn` only for FAILED (fresh session, `retry_ordinal`+1); `cancel_turn` settles only after stop observed; UserRequest resolution moves `WAITING_USER→WAITING_DEPENDENCY`, `PAUSE_REQUESTED` returns `CONFLICT`. Says "storage context reader is implemented, but this service call, supervisor handoff, response writer, usage writer, and usage query are not connected in the current build."
- `AgentTurnCoordinator.continue_from_user_request` — fresh Conversation-scoped session, no aggregate writes, `conversation.turn.resumed.v1` only after session ready.
- RoutineService `run_now` commits one ordinary standalone `READY` Task (no planning/execution, rejects Conversation origin, cannot carry grants/credentials/handles); `archive` blocked while enabled Automations point at the revision.
- AutomationService owns definitions only (`TRIGGER_UNSUPPORTED` on unsupported enable); TriggerCoordinator `ingest/claim/request_manual_occurrence/advance_cursor/materialize_task/settle_occurrence` with `claim_epoch` fencing. **"The current local owner ManualTrigger slice … does not claim scheduled/provider trigger hosting, cursor advancement/recovery, or Task-outcome settlement support"** — behind AUTOMATION.md's landed settlement producer.
- `ConversationService.open_task_follow_up(TaskId)` returns the one `PRIMARY_FOLLOW_UP` Conversation, idempotent, never invents old messages, may reuse a qualified native resume point.
- LearningService: `observe_user_message/observe_task_outcome/observe_feedback/observe_demonstration`, `classify`, `approve/dismiss/reject`, `auto_commit_eligible(CoworkerId, batch_limit)`, `reconcile_conflicts`; "Only policy-qualified private memory can use the quiet automatic commit path."
- ArtifactRuntimeSupervisor: volatile run supervision only, `prepare` is inert, no Task/Evidence/DomainEvent, `SAVE` calls ArtifactStore.create for a separate FILE Artifact; STOPPED only after process tree + broker handles are gone, else `INTERRUPTED`.
- EnvironmentManager preview TTL default 5 minutes, single-use, digest bound to principal/incarnation/offer/quote; `REQUIRE_PROVIDER_ENFORCED` rejects when the provider cannot enforce the ceiling.
- ResourceUploadService: 100 MiB cap, 4 MiB chunks, 24 h expiry, sweep ≤100 sessions/30 s; ArtifactStore rejects `input_refs`≠`ProvenanceRecord.source_inputs` with `INTEGRITY_FAILURE`.
- Transaction rule: validate → recheck versions/auth/fence inside tx → commit aggregate+events+receipt together; "External network/provider calls never run while holding a StateStore transaction"; long Forbidden-edge list (no UI→StateStore, no adapter writing domain tables, no projection becoming authority, no warm manager starting model work).

### docs/adr/0002-task-and-attempt-identity.md (21 lines)
- Status: Accepted for vNext, 2026-10-01. Task is durable; Step is semantic work; Attempt is one execution try; AgentSession/Runtime/Environment are independently replaceable.
- Handoff reconciles Effects, checkpoints portable Task state, transfers a fenced lease, creates a new Attempt.
- Consequences: resume cannot depend on native session snapshots; ambiguous non-idempotent Effects block automatic retry until reconciled or reviewed.

### docs/adr/0003-litespm-is-independent.md (30 lines)
- Accepted for vNext, 2026-10-01. LiteSPM owns discovery/verification/install/update/removal and provider supervision for connectors, MCP servers, plugins, skills.
- LiteCowork owns CapabilityRefs, Offers, Grants, Activations, Invocations, Effects, Evidence; integrates through a LiteSPM client.
- Consequences: "Catalog discovery and package installation grant no execution authority"; active Tasks pin exact versions/digests; no second marketplace; no silent rewrite of native extension config; per-agent attachment route must be adapter-qualified.

### docs/adr/0013-host-delegation-uses-accepted-plan-steps.md (25 lines)
- Accepted for vNext, 2026-10-05. Child Attempts only for `READY` Steps in the current accepted immutable PlanRevision; the lead proposes a revision through the normal Task path.
- Each child gets a new AgentSession, lease, budget reservation, freshly evaluated scoped grants, Environment attachment under its sharing/isolation contract; "Parent grants, approvals, SecretLeases, and native credentials never flow to the child".
- Consequences: auditable/recoverable child work; delegation may require a plan revision before execution; retries/escalations create new Attempts, worker identity never mutated in place.

### docs/adr/0022-rich-presentation-is-optional.md (33 lines)
- Status: Accepted. `ConversationMessage` stays semantic truth and commits/renders without waiting for rich compilation.
- A bounded immutable version-1 `RichPresentation` keyed to exactly one committed Agent message + semantic digest carries no authority; rich-only status/citations/deliverables/actions/facts are forbidden; no Message v2 and no per-interaction DomainEvents.
- Missing/delayed/unsupported/corrupt presentation falls back to the complete semantic message; presentation may upgrade a rendered message in place; no replay of historical typing/motion.
- Consequences: older Operators/channels stay readable; RichPresentation storage/backup/GC/replication is independently versioned and integrity-checked (matching RICH-RESPONSE's 512 KiB/depth-6 bounds).

## Cross-cutting observations
- **Qualified vs designed:** the entire bucket is design-tier. 0 of 25 `FL` chains, 0 of 14 `F-C` journeys and 0 of `UIV1-001–158` carry QUALIFIED evidence. `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md` states "No operationId is marked SERVED"; `FL` ledger says native Send and browser takeover are not implemented.
- **Only CODE-tier claims in the bucket:** the AUTOMATION settlement producer (`SqliteAutomationHostStore::settle_started_occurrences_from_task_outcomes`, 2026-10-22, still release-blocked on cursor retirement + OS notification delivery); Windows root-reopen identity tests ("partial CODE evidence only"); DESIGN-SYSTEM's 11 base tokens and its long list of token/state gaps; SCHEMAS' narrowed current agent enums; SERVICES' "storage context reader implemented, writers not connected".
- **Settlement-ownership drift:** AUTOMATION.md (2026-10-22) claims the Task-outcome settlement producer is landed and served, while SERVICES.md still says the local ManualTrigger slice "does not claim … Task-outcome settlement support". AUTOMATION is the domain owner; SERVICES needs the update.
- **Workbench-open contradiction:** COWORKERS-TARGET §1 ("visible at launch in its compact Start state") vs its own §14 ("not always open by default; new chats start with maximum conversational width"). DESKTOP-SHELL-LLD and COVERAGE-MATRIX follow §1, so §14 is the stale half.
- **Count drift:** "35 UI interaction groups" (V1-UI-ACTION-STATE-LEDGER, DESKTOP-SHELL-LLD) vs "36 shell controls in the action ledger" (CONTRADICTION register). Same settings ledger: 187 controls / 30 groups consistently.
- **Registry drift inside the bucket:** SCHEMAS.md uses `CONVERSATION_CONTEXT_LIMIT_EXCEEDED` for the 1,024/4 MiB package cap, but the code is absent from the prose error registry in the same file and the register says it is not in the 131-code registry. Similarly `ConnectionStatus` omits `REVOKED` here while the register records that omission as known drift.
- **One-memory-owner discipline:** COWORKERS-TARGET, SCHEMAS, SERVICES and HOST-GUIDANCE all refuse to create a second memory state machine — LEARNING.md owns candidate states/events, ResourceService owns committed ContextDocuments, `require_user_confirmation_for_memory` is legacy, no `AUTOMATIC_WITH_APPROVED_SHARING`, and `PersonalContextProvider` deliberately has no `propose_memory`.
- **Truthfulness motif repeated everywhere:** PENDING only from real commands, unknown never rendered as zero/$0/available, agent "done" never equals verified, toast only after receipt, no fake Chrome/cursor/ETA/confetti, missed local slots recorded as missed ("Never claim 24/7").
- **Authority-order variance:** COVERAGE-MATRIX's 7-level order omits `docs/IMPLEMENTATION-MASTER-PLAN.md` and the Rev 9 master spec that the repo-root instructions place at levels 6–7; ADRs are placed last there ("never override"), consistent with root guidance.

## Coverage check
- All 19 listed files read to the end. `docs/COWORKERS-TARGET.md` was first returned truncated around §13F and completed with two offset reads (lines 368–444); `docs/SCHEMAS.md` and `docs/SERVICES.md` were paged in three passes each and both ended at their final "V1 detailed design integration — 2026-10-12" section.
- A repo-wide grep for `UC-*` identifiers was cut off by the deadline; within the 19 files read, no `UC-*` identifier appears (verified in `V1-END-TO-END-FLOW-AND-RACE-CONTRACTS.md` by grep, and by reading the remaining files). Treat "no UC IDs in this bucket" as bucket-scoped, not repo-wide.
- Files referenced but outside this bucket and therefore not verified here: `docs/STATE-MACHINES.md`, `docs/EVENTS.md`, `docs/LEARNING.md`, `docs/ROUTINES.md`, `docs/TRUST.md`, `docs/ARTIFACTS-EVIDENCE.md`, `docs/V1-SETTINGS-CONTRACT-LEDGER.md`, `docs/schemas/error-codes.schema.json` (the 131-code registry that would settle the `CONVERSATION_CONTEXT_LIMIT_EXCEEDED` question).
