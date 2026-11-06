[Reading 162 lines from start (total: 162 lines, 0 remaining)]

# 011 - Desktop Shell IPC Settings and Runtime Health

**ID:** 011
**Status:** DRAFT
**Owners:** Runtime, Operator IPC, Desktop UI, Product
**Updated:** 2026-11-06
**Revision:** 0.2
**Target release:** V1 local desktop
**Dependencies:** root ARCHITECTURE.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md, docs/IMPLEMENTATION-MASTER-PLAN.md, and the owner documents named in RESEARCH.md. Feature 001 remains the Conversation send authority.

## 1. Problem

The shell is not the security boundary. A settings toggle without a handler must not look armed. Window close, sleep and daemon stop are different. WSL is not native Windows or Linux qualification.

Observable outcome: The owner launches the desktop, reaches a real local Runtime over authenticated IPC, changes a setting only when its handler exists, and sees truthful runtime health across restart.

## 2. Goals

- Authenticate Tauri-to-daemon IPC without a loopback bearer.
- Map each settings control to LOCAL_ONLY, NATIVE, TARGET or a served command.
- Expose Runtime incarnation, readiness and drain state honestly.
- Keep semantic chat usable when rich presentation is disabled.

## 3. Non-goals

- Replacing the daemon with a window-owned Electron host.
- Cloud Runtime enrollment.
- Claiming all 187 settings are served.
- Mobile clients.

## 4. Current evidence

The active source tree is present in apps/ and crates/. The behavior below is the normative feature target; source, served-route and UI evidence for this checkout is recorded in docs/IMPLEMENTATION-STATUS.md and docs/SETTINGS-AND-SHELL-SURFACE-AUDIT.md. Code or a route alone does not qualify the feature. This package remains DRAFT until its named owners accept the decisions and supporting contracts.

### Current source crosswalk — 2026-11-06

The active App.tsx Settings route mounts `SettingsView.tsx`, with four visible categories only: General, Workspaces, Agents and Runtime. The 16-category IA later in this SPEC is the V1 target, not current navigation. The mounted view currently exposes Workspace selection/refresh/create, immutable Workspace instruction revision editing, archive, bounded Agent catalogue/profile probes and binding operations, and Runtime start/refresh/lifecycle observation. It does not expose WorkspaceRoot management, Workspace rename/policy mutation, Runtime enrollment, Runtime stop/drain or startup-policy mutation. General appearance controls are ephemeral component state and do not apply/persist preferences; the source comment claiming localStorage persistence is inaccurate. Shortcut labels are not wired to key handlers. There is no Settings search or responsive category drawer in the active component. The Workspaces & Roots heading overstates the visible operations; the panel shows replication policy read-only and does not expose roots or policy changes. The Agents heading mentions OpenCode, Codex, Claude Code and Gemini, but active probes are only Codex and OpenCode; the 15 curated candidates/ACP distributions are catalogue metadata, not detected/installed/turn-capable agents. The alternate exported `SettingsPage` in App.tsx is unmounted. Earlier nine-category and Browser/cache/TLS/Shell Engine/approval/Credential Store claims are stale; Browser/Terminal are Workbench surfaces owned by Feature 008. The current UI's “Connected (Localhost)” copy is inaccurate for OS-local socket/named-pipe IPC. The mounted component also fails to render Runtime error/loading props, Workspace-create pending-journal retry state and Workspace-instruction conflict recovery controls that are passed into it. App bootstrap separately attempts auto-start when no daemon process is running and the daemon executable is available. See the source audit for exact controls and limitations.

## 5. LOCKED and FLEXIBLE decisions

### LOCKED

- Operator IPC does not change API authorization (LOCAL-OPERATOR-IPC).
- Principal comes from the authenticated transport, never a client-supplied id (settings ledger).
- Display preferences must not restore grants.
- Three independent lifecycles: Runtime, AgentHost, Environment (RUNTIME-LIFECYCLE).
- V1 is local-first desktop (ARCHITECTURE).
- Untrusted presentation cannot forge host actions (master plan P10, owned here as a shell rule).

### FLEXIBLE

- Module file layout and private helper names, provided the transactions and secret boundaries stay intact.
- Presentation copy, provided state distinctions remain visible.

### Open decisions

- GA OS order.
- Which settings groups are in the first served milestone versus remain TARGET.

### Blockers

- B-28: GA OS and IPC qualification bar.
- B-29: first settings groups allowed to be enabled in the shell.

## 6. High-level design

Tauri shell renders. IPC carries Operator commands to litecoworkd. Runtime lifecycle owns incarnation and drain. Settings UI consults the semantic map: LOCAL_ONLY may change immediately; TARGET stays disabled with the ledger's failure text; served commands wait until a later package marks them SERVED.

Trust boundary: the desktop UI is untrusted; the authenticated Operator is the only command path; native credentials and OS handles stay outside public events; SQLite transactions do not perform network or process calls.

## 7. Low-level design

No new operationIds. Health reads use existing Runtime readiness contracts. Settings mutations use idempotency and If-Match only when the ledger class is OPERATOR_CAS and the operation is actually mounted. This package's implementation task is the shell gate plus IPC auth tests, not silently mounting all settings.

Ordering: authorize, compare expected version, commit the durable receipt, then perform any external call, then reconcile. Proposed payload fields are PROPOSED until the canonical owner document and machine schema change together.

## 8. Data

See [DATA-MODEL.md](DATA-MODEL.md). This feature does not create a second source of truth beside the domain owner.

## 9. Wiring

See [WIRING.yaml](WIRING.yaml). Requests enter through the Operator. External calls sit outside transactions.

## 10. Security

Peer credentials or named-pipe DACL as already specified. Reject remote pipe clients. Redact IPC frames in logs. Do not trust WebView-supplied principals.

## 11. Errors and recovery

Daemon crash shows disconnected, not an empty successful workspace. Restart creates a new incarnation and reconciles hosts per RUNTIME-LIFECYCLE. Drain stops admission before claiming stopped.

Races use compare-and-swap or a single-writer occurrence identity. Timeout after an external effect reconciles. Duplicate retries return the original receipt when the idempotency key matches and fail when the payload conflicts.

## 12. Crash and restart

Reload the last committed owner records. Do not infer success from an intent. Reconcile in-flight external work before a new side effect. Cancellation fences late results.

## 13. Configuration and compatibility

Per-OS qualification is separate. Accessibility: keyboard path for New Chat, Settings and disabled-control explanations.

OS qualification is per native desktop. WSL2 does not qualify Windows or Linux GUI behavior.

## 14. Observability

Log ids, versions, statuses and denial reasons. Do not log secret bytes, native handles, private locators or raw credentials. Audit commits include actor, workspace and receipt id.

## 15. EARS acceptance

- **AC-001:** WHEN the shell sends a command, THE SYSTEM SHALL accept it only from an authenticated local IPC peer.
- **AC-002:** IF the peer marker is missing, THEN THE SYSTEM SHALL reject the command.
- **AC-003:** WHEN a settings control is TARGET or unserved, THE SYSTEM SHALL render it unavailable and SHALL NOT send a mutation.
- **AC-004:** WHEN a control is LOCAL_ONLY, THE SYSTEM SHALL persist presentation preference without changing grants or bindings.
- **AC-005:** WHEN the Runtime incarnation is STOPPED, STARTING, or RECOVERING, THE SYSTEM SHALL show that health state and SHALL NOT present execution as ready. WHILE the incarnation is DEGRADED and GET /v1/operator/readiness is SERVING, THE SYSTEM SHALL still allow ordinary Operator commands and SHALL keep execution readiness and Task recovery closed.
- **AC-006:** WHEN the window closes and the Runtime is still running, THE SYSTEM SHALL NOT report the Runtime as stopped.
- **AC-007:** IF rich presentation fails, THEN THE SYSTEM SHALL still show semantic Conversation text.
- **AC-008:** WHEN tests run only under WSL2, THE SYSTEM SHALL NOT record them as native Windows or native Linux desktop qualification.

## 16. Executable acceptance command

```bash
python3 .specify/scripts/validate_feature_package.py specs/011-desktop-shell-ipc-settings-runtime
```

Further gates after ACCEPTED/LOCKED are the cargo filters named in TASKS.md. They are not evidence until run on a recovered source tree.

## 17. Out-of-bounds

- Do not mount every OpenAPI route in this package.
- Do not implement cloud continuation.

## 18. Assumptions and open questions

- Historical IPC CODE in the archive is not current served evidence for this checkout.
- Semantic map rows start as TARGET or LOCAL_ONLY until source returns.

Each blocker has the owners named above. Resolution gate: owner decision recorded in this SPEC before status leaves DRAFT. Implementation tasks: 011-T01, 011-T02, 011-T03, 011-T04.

## 7a. Owner facts incorporated 2026-11-01

Read from LOCAL-OPERATOR-IPC and RUNTIME-LIFECYCLE.

- IPC limits: 16 KiB header, 100 MiB request body, 10 MiB response body, 8 concurrent accepted requests, 128 MiB combined in-flight bodies, 3-second connect and header deadline, 30-second body-read deadline, handler deadline at most 60 seconds.
- Logical request headers are only X-Workspace-ID, Idempotency-Key, If-Match, Content-Type, Content-Range, and X-Chunk-SHA256. Authorization, Host, Origin, cookies, and hop-by-hop headers are forbidden. The frame request_id is correlation only and is not an Idempotency-Key.
- Unix sockets are owner-only 0600 in an owner-only 0700 directory. Symlinks are rejected. The peer UID must match the installation binding. Windows uses a local named pipe whose DACL grants only the current logon SID and SYSTEM. Remote clients are rejected. If that DACL cannot be created, startup does not open an unauthenticated pipe and does not fall back to loopback bearer authentication.
- Local incarnation recovery_state moves through STARTING, RECOVERING, READY, DEGRADED, DRAINING, STOPPING, and STOPPED. That is not the payload of runtime.availability.changed.v1, whose schema enum is only PAIRING, ONLINE, DEGRADED, DRAINING, OFFLINE, and REVOKED. The lifecycle view availability enum also includes STARTING and RECOVERING, so those three lists stay distinct. GET /v1/operator/readiness confirms Operator API serving for the reported incarnation and is not execution readiness. GET /v1/runtime-lifecycle does not probe providers or change lifecycle state.
- Graceful drain stops admission before persisting DRAINING, allows admitted handlers at most 60 seconds, and reaches STOPPING and STOPPED only after those handlers finish. A handler deadline is unsettled because work may continue after HTTP 504. The authenticated Operator stop route stays absent until Task, Effect, lease, and local-service settlement exist.
- RuntimeStartupPolicy is MANUAL, LOGIN_BACKGROUND, or ALWAYS_ON_SERVICE. LOGIN_BACKGROUND is the recommended personal-desktop mode. Sleep suspends the local Runtime. Stopping the Runtime is distinct from cancelling Tasks.
- After restart, a failed identity check makes an ACTIVE root UNAVAILABLE, leaves a PAUSED root PAUSED, and does not reactivate a revoked root.
- Cite getLocalOperatorReadiness, getLocalRuntimeLifecycle, enrollLocalRuntimeInWorkspace, previewLocalRuntimeStop, requestLocalRuntimeStop. Event: runtime.availability.changed.v1.



## 19. Requirement traceability

| Requirement | Spec | Canonical contract | Task | Acceptance |
|---|---|---|---|---|
| AC-001 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T01 | AC-001 |
| AC-002 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T01 | AC-002 |
| AC-003 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T04 | AC-003 |
| AC-004 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T04 | AC-004 |
| AC-005 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T03 | AC-005 |
| AC-006 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T03 | AC-006 |
| AC-007 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T04 | AC-007 |
| AC-008 | EARS acceptance | canonical owner docs cited in this SPEC | 011-T04 | AC-008 |

[executed on device: DESKTOP-SARV (a709d51c-bcd7-4e97-a12f-b3b065c93231)]
## V1 Settings screen map and control behavior — 2026-11-06

Feature 011 owns the shell/navigation and Settings host. Each category continues to defer to its existing domain owner and [V1 Settings Per-Control Authority Ledger](../../docs/V1-SETTINGS-CONTRACT-LEDGER.md). This section selects the V1 information architecture and makes explicit that the target categories do not mean all underlying controls are served.

### Shell layout
Persistent category navigation on the left, focused content panel on the right. Compact Settings heading and local search at top. Categories are grouped, not rendered as dozens of top-level tabs. Category selection preserves Workspace/Runtime scope and search query. Narrow windows use a focus-managed category drawer and detail back navigation. Local category layout/search preferences do not persist grants, credentials, auth or active process handles.

### V1 categories in order
1. General — appearance, language, density, text scale, reduced motion, keyboard shortcuts.
2. Notifications — sound, OS notification delivery, quiet hours, local preview.
3. Workspaces — selection, create, rename, roots, instructions, archive, default lead.
4. Agents — Catalog, Installed, Profiles, Workspace bindings/defaults, Usage shortcut.
5. Connections & Tools — Discover/LiteSPM package lifecycle and Connections only where their owner operations are qualified.
6. Local Models — Runtime-specific local-server observation and exact AgentModule compatibility; no global model router.
7. Coworkers & Subagents — Coworker defaults/delegation, worker profiles, concurrency and budget settings under their owners.
8. Runtime — daemon health, readiness, startup policy, recovery, active environments.
9. Terminal — shell profiles, terminal preferences, retention and health; no interactive service controls unless daemon-owned PTY is qualified.
10. Browser & Computer — managed profiles/permissions/session and OS control only when a real qualified provider exists.
11. Memory & Knowledge — scoped sources, memory policies and revocation only when memory/context owner operations exist.
12. Security & Privacy — grants, secret presence, egress, retention, revocation.
13. Storage & Backup — usage report, safe cleanup preview, backup/export/import/restore.
14. Usage & Monitoring — usage summaries/drilldown, quota observations, latency/failure and source coverage.
15. Advanced — OS keep-awake assertions, restart/recovery policy and advanced diagnostics.
16. About & Diagnostics — version/build, redacted logs, provider diagnostics, support docs/bundle.

Search searches settings labels/help and only the currently available configuration surface. It must not expose secrets or let a text match enable an unavailable control. Every setting row shows its scope (Global, Workspace, Coworker, AgentBinding, Runtime or local OS) and whether saved state is local, native-observed, Operator-committed, read-only or unavailable.

### Notifications tab
Use four mutually exclusive delivery combinations: Sound + notification; Sound only; Notification only; Neither. Quiet hours show explicit timezone and selected notification classes. "Try it" plays a local sound preview and may request an OS notification only if the OS notification provider is implemented; its result reports each channel separately. Notification delivery queue is not Needs You: Needs You reflects unresolved actionable UserRequests/approvals, not all unseen notifications. No "Off" summary may hide which channel is disabled.

Buttons: select each delivery combination; edit quiet-hours start/end and timezone; choose supported classes; Try it; Reset to defaults. Each is LOCAL_PREF unless an OS notification delivery operation is explicitly owned and served. Preview never creates a domain event or external delivery receipt.

### Advanced tab
Keep-awake choices: Off; While agents/work run (including battery); While agents/work run on power only. These require native OS power assertions and must expose unsupported/unavailable state per OS; do not infer behavior from a selected radio option. "Resume agents after restart" is not generic process resurrection: it applies only to LiteCowork-owned jobs with durable admission and a qualified recovery contract. It does not restore arbitrary third-party native CLI TTY/session state. Runtime startup policy (MANUAL, LOGIN_BACKGROUND, ALWAYS_ON_SERVICE) is independent from resume policy. Any control affecting active work must show the effective boundary, pending/restart-required status and what it cannot resume.

Buttons: keep-awake option, resume admitted jobs toggle, Inspect recovery, Start runtime/refresh status where served, Preview stop/drain, Stop safely only when lifecycle owner admits. No hardcoded checkmarks or locally flipped "On" state for OS/Runtime actions.

### Agents tab
See Feature 002's full Agent catalog/profile contract. Settings shows a searchable catalogue and independent lifecycle chips; installed agents and available distributions are separate lists; selected item opens a profile with Overview, Installation, Authentication, Capabilities, Workspace bindings and Diagnostics. No global "Provider" page or generic model dropdown. Native account/login/model configuration is harness-owned; LiteCowork records redacted observations and scoped binding/default choices.

### Usage & Monitoring tab
Use the canonical telemetry LLD. The top-level quick ranges are Today, Last 7 days, Last 1 month, Last 3 months, Last 1 year and All time; YTD and Custom are expanded options. Primary filters include agent/harness, model, provider, AgentBinding, Coworker, Conversation/session, Task, Attempt, lead/subagent, Runtime, source/confidence and outcome. Grouping supports day/session/Coworker/Task/agent/model/provider. Scope defaults to selected Workspace; all-workspace totals require explicit scope.

Display requests and input/output/cached tokens with coverage; estimated list-price equivalent separate from provider-billed amount; currency partitions; quota snapshots and reset windows only where reported; latency/failure and source coverage. Detail table opens to leaf inference records and provenance, never prompt/response text. Refresh is a read operation. Export is a user-initiated bounded, redacted local file. No billing account management, price editing, budget enforcement or quota reset is implied.

The canonical telemetry LLD owns the data model and explicitly states that current dashboard projections are target-only. Therefore V1 Settings must show an honest unavailable/empty-with-coverage message until the complete Operator + SQLite + schema + UI projection exists. Missing usage is UNKNOWN, never zero. Provider-reported/billed, native/gateway reported, tokenizer estimate and unknown remain distinguished. Flat-plan included use is not $0 API spend.

### Other Settings controls
Every row/action remains subject to the per-control ledger. Appearance and layout controls may be local preferences. Workspace operations are versioned Operator mutations. Profile/binding actions use Feature 002. Connection install/use uses LiteSPM and Trust. Runtime actions use Runtime lifecycle. Keep-awake uses OS integration. Terminal actions require TerminalSessionService. Browser/Computer settings require qualified native providers. Storage cleanup and restore require preview + exact impact + version/receipt. Diagnostics are scoped reads or explicit redacted exports.

Do not surface all target controls in V1 as active UI. A control with no served operation is absent or visibly unavailable with a concrete missing prerequisite; no fake toggle, inert ellipsis menu, success toast before receipt, or state derived from a click.
