# LiteCowork V1 — Product-to-Implementation Acceptance Matrix

**Status:** active release-evidence crosswalk. **Not a pass certificate.**  
**Sources:** [ten-area contract](../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [UI contract](../docs/V1-UI-SCREEN-CONTRACT.md), [LiteSPM interop](../docs/LITESPM-LITECOWORK-INTEROP-LLD.md), [native HorizonCode](../docs/V1-HORIZONCODE-NATIVE-AGENT-LLD.md), [existing qualification](QUALIFICATION-MATRIX.md), [served routes](served-route-inventory.json).

## Truth labels

- **TARGET:** normative HLD/LLD only; no served behavior implied.
- **CODE:** compiled/unit exercised code; may be unreachable from real app.
- **SERVED:** authenticated route and storage/source path mounted; provider/runtime may still be unusable.
- **INTEGRATED:** real provider + desktop + persisted path proven for exact version.
- **QUALIFIED:** CODE, SYSTEM and owner USER evidence on exact OS/build/version, including negative recovery.
- **BLOCKED:** required V1 flow has no qualifying end-to-end evidence. A future pass must link evidence rather than overwrite this label without explanation.

These labels are the same axes as the [release contract](../docs/V1-PRODUCT-RELEASE-CONTRACT.md) vocabulary, spelled one step shorter: **TARGET** = `TARGET_ONLY`, **CODE** = `CODE_IMPLEMENTED`, **SERVED** is identical, **INTEGRATED** = `INTEGRATION_PROVEN`, **QUALIFIED** is identical, and **BLOCKED** is this matrix's release verdict for a journey whose evidence chain is incomplete — it is not a sixth implementation axis. Never relabel a row without the evidence the target vocabulary names.

## Required journeys and current release verdict (2026-10-22 review)

| Area | User-visible release journey | Domain/source owner | Evidence / release state |
|---|---|---|---|
| Chat | Fresh install → choose qualified native lead → authenticate → send/stream/stop → reload persisted history | E03-S01/S02; `conversation_operator.rs`, `ConversationsPage.tsx` | **BLOCKED** — composer Send disabled and native session admission refuses |
| Native agent | Detect, configure native account/options, start a real native session, preserve native existing config | E03-S01/S06–S11, Agent Fabric | **BLOCKED** — descriptor/profile modules are not qualified sessions |
| Coworker | Quick Create → first usable chat → scoped context/worker/grant → restart | E08-S01/S03; domain-responsibility | **BLOCKED** — underlying real chat/memory path unavailable |
| Computer/Workbench | Real browser/computer/PTY work with takeover, return and observation | E07-S02/S05, E08-S04/S07 | **BLOCKED** — full provider/platform SYSTEM not qualified |
| LiteSPM | Observe native MCP → preview adopt → unchanged config → assign/grant → invoke → unmanage | E04-S02/S06, E10-S01; LiteSPM service | **BLOCKED** — upstream machine contract and real integration not qualified |
| Task | Request durable outcome → accept plan → admit Attempt → perform/verify artifact → restart safely | E03-S03, E04-S03/S05, E07-S00 | **BLOCKED** — admission/lease proof producers not integrated |
| Multi-agent | Chief delegates one bounded worker Step → verified integration/handoff | E05-S01–S05 | **BLOCKED** — native/host child execution and safe lead switch not qualified |
| Automation | Schedule → exact occurrence → Task → settle/notify → recover from sleep/restart | E09-S01–S05, Runtime | **BLOCKED** — Task execution is still unqualified, recurring trigger hosting and OS notification delivery do not exist, and the settlement producer is CODE (storage + served route tests) with no owner USER evidence |
| Memory | Scoped source → grounded retrieval → eligible learning → review/revoke → no cross-scope leak | E06-S02/S03, E08-S03 | **BLOCKED** — full producer/retrieval/use path unqualified |
| Artifacts | Real source → create supported doc/spreadsheet/code → preview/edit/version/export/reopen | E04-S05, E08-S07, E07-S04 | **BLOCKED** — format/provider-specific SYSTEM paths unqualified |
| Desktop/release | Installer, accessibility, backup/restore, recovery, diagnostics, performance by OS | E02-S01, E13-S01–S04 | **BLOCKED for V1 GA** — individual code slices are not whole-platform proof |

These are product-level release verdicts, not a claim that their underlying isolated source components have no passing tests.

## Risk and owner ledger

| Priority | Exact corrective item | Primary file/epic |
|---|---|---|
| P0 | Chat-first HLD + no mandatory Task/Coworker first use | `ARCHITECTURE.md`, PRODUCT/EXPERIENCE, E02/E03 |
| P0 | Real native Session/Send instead of disabled control | E03-S01/S02; `conversation_operator.rs`; `ConversationsPage.tsx` |
| P0 | Real plan/Attempt admission and durable receipts | E03-S03, E07-S00, E04-S03/S05; `storage-sqlite/src/execution.rs` |
| P0 | LiteSPM interop/versioned inventory and plan/apply authority | `LITESPM-LITECOWORK-INTEROP-LLD.md`; `discover_operator.rs`; upstream LiteSPM |
| P1 | HorizonCode dedicated AgentModule and worker conformance | `V1-HORIZONCODE-NATIVE-AGENT-LLD.md`, E03/E05 |
| P1 | Discover mounted and real package use/management UI | `App.tsx`, `src/discover/` |
| P1 | Real Browser/Terminal/Computer provider, Workbench and format writer qualification | E07/E08 |
| P1 | Scope-respecting memories and real Knowledge retrieval | E06/E08 |
| P1 | Recurrence occurrence settlement from a terminal Task outcome: producer and served route are CODE-implemented; owner USER evidence still pending | `automation_host.rs` (`settle_started_occurrences_from_task_outcomes`), `occurrences.rs` |
| P1 | Trigger-host cursor retirement on disable, plus real OS notification delivery (attempts, `notification.delivery.changed.v1`, OS bridge) | `workspace_archive.rs`-adjacent disable path, `notification.rs` |
| P1 | Old nonexistent implementation-file references removed or target-labelled | `CONNECTORS-WORKFLOWS.md`, `LEARNING.md`, review packets |
| P1 | Fix docs claiming LIVE for unserved operation/events | `API.md`, `EVENTS.md`, `STORAGE.md`, generated route inventory |
| P2 | Consistent packaging/free local core and true format capability labels | PRODUCT, SCOPE, ARTIFACTS, E13 |

## Per-journey remaining work — explicit, 2026-10-22

Each area states, without abbreviation: what exists today (with source evidence), exactly
what must still be built (named files/routes), and the evidence that flips the verdict.
Nothing below is claimed done.

### Chat — verdict BLOCKED

**Exists.** Conversation catalog is served and durable: `POST/GET /v1/conversations`,
`GET /v1/conversations/{id}`, `GET /v1/conversations/{id}/presentation`,
`GET /v1/conversations/{id}/tasks` (`conversation_operator.rs:101-220`,
`presentation_operator.rs:38`, `operator.rs:1615`). `POST /v1/conversations/{id}/turns`
is served but cannot dispatch: the handler commits the USER message and OPEN turn, then
the storage layer refuses every `TurnCommand::Start`
(`crates/storage-sqlite/src/conversations.rs:398-400` and the duplicated event-validation
gate at `:590-594`), so the handler settles the turn `FAILED` with
`CONVERSATION_SESSION_ADMISSION_UNAVAILABLE` and returns 503
(`conversation_operator.rs:441-508`, `:766-772`). Retry/cancel always return 503
(`:537-571`). The desktop composer renders an unconditionally `disabled` Send with no
handler (`ConversationsPage.tsx:583`) and derives `NEEDS_REFRESH` from a hard-coded null
descriptor (`:162-174`). The stream client is complete
(`presentation-stream.ts:194-314`) and the Tauri bridge subscribes to all three streams
(`conversation_stream_bridge.rs:329-365`), but no turn/send/stop command exists in the
bridge and the local Operator emits no `turn.delta`/`turn.settled`/`rich.draft`.

**Remains.**
1. `crates/storage-core`: a conversation-scope admission commit type (session record +
   turn + events + receipt) next to `CreateConversationTurnCommit`.
2. `crates/storage-core` `AgentSessionStore`: conversation-scope methods; today every
   method is planning-only and `start_task_planning_session` structurally rejects a
   `conversation_id` (`lib.rs:1632-1626`, gate `lib.rs:245-251`).
3. `apps/litecoworkd/src/agents/`: a turn orchestrator that resolves the binding, opens a
   Conversation-scoped session on the existing `CodexAppServer`
   (`codex_app_server.rs`, `thread_start`/`turn_start`/`turn_interrupt`/`poll_event`) or
   `OpenCodeServer` (`opencode_server.rs`, `create_session`/`send_message_async`/
   `subscribe_events`/`abort_session`), and maps native events to committed messages.
4. `crates/storage-sqlite/src/conversations.rs`: replace the two `Start` refusals with
   real admission — insert the `scope_kind='CONVERSATION'` `agent_sessions` row (the v1
   schema already supports it), bind `conversation_turns.agent_session_id`, and add the
   AGENT-message writer that is missing today (only the USER insert exists at `:269`).
5. `conversation_operator.rs`: return the OpenAPI-promised `202 ConversationTurnReceipt`
   and implement retry/cancel against `TurnCommand::Retry`/`RequestCancel`.
6. A daemon producer for the three transient frame types, or the desktop must keep
   reporting no live frames.
7. `apps/litecowork-ui/src-tauri`: `submit_conversation_turn`, `stop_conversation_turn`
   commands over the existing authenticated IPC.
8. `ConversationsPage.tsx`: Send/Stop/Retry driven by real turn state, a real descriptor
   and binding, with a visible reason whenever disabled.
9. `codex_profile_probe.rs`: a path to conversation eligibility (today
   `current_conversation_safety_assessment` hard-sets every control to `Unknown` →
   `NOT_ELIGIBLE`, so dispatch would refuse even after 1-8).

**Evidence to flip.** CODE: storage admission + orchestrator + operator tests. SYSTEM: one
real Codex or OpenCode turn on an exact pinned version, proving the committed AGENT
message, the turn receipt, replay-with-same-key writing nothing, and stop/cancel settling
the turn. USER: owner sends, streams, stops, reloads, restarts the daemon mid-turn and
sees the honest settled state.

### Native agent — verdict BLOCKED

**Exists.** Local discovery for 25 curated candidates with `--version` probes
(`agents/mod.rs:321-371`, `native_cli.rs`), Codex/OpenCode profile probes behind
`POST /v1/agent-profiles/probe`, control descriptors, `AgentBinding` CRUD with
enable/disable/default, and a settings surface that is explicitly display-only
(`App.tsx:818`).

**Remains.** Per-agent qualification stories E03-S06 through E03-S11 and E04-S06 (exact
agent version × OS × provider), native configuration preservation, credential path per
harness without LiteCowork copying secrets, `AgentInstallationState`/`AgentAuthState`
observation states whose producers do not exist yet (`docs/SCHEMAS.md` status enums), and
the HorizonCode module path (`V1-HORIZONCODE-NATIVE-AGENT-LLD.md`).

**Evidence to flip.** CODE: session lifecycle admission/activation/loss per harness.
SYSTEM: real session started, used, interrupted and lost on the exact binary. USER: owner
authenticates their own account, runs a native session, restarts and reuses it.

### Coworker — verdict BLOCKED

**Exists.** Roster CRUD with pinned revisions, pause/archive/primary selection, worker
-profile validation, and an editor surface (`coworkers/CoworkerSettings.tsx:492-667`);
domain rules and 99 domain tests.

**Remains.** A Coworker-owned Conversation path (chat-first, no Task), the scoped memory
list with provenance, connection assignment/revoke UI, and the Quick-Create → first usable
chat flow end to end. Memory policy text in the editor is honest that it is not active.

**Evidence to flip.** CODE: Coworker conversation admission + scoped memory projection.
SYSTEM: create Coworker, chat, assign a worker, restart. USER: owner completes the flow
with their own content and confirms no cross-Coworker leakage.

### Computer/Workbench — verdict BLOCKED

**Exists.** Nothing user-facing: no Workbench shell, no browser/computer/terminal surface,
no takeover controls. The presentation Browser card is informational only
(`PresentationRuntime.tsx:59`). Linux Environment isolation primitives exist
(`crates/linux-process-scope`).

**Remains.** Qualified Linux Bubblewrap Environments (Windows/macOS stay unsupported in
V1), a Chrome provider with explicit consent and takeover/return epochs, a real PTY bound
to Environment/Trust ownership, the right-panel shell (Activity/Browser/Computer/Terminal/
Files/Artifact) with focus and unsaved-edit rules, and per-format artifact writers.

**Evidence to flip.** SYSTEM: real Chrome attach/takeover/return and a real PTY session on
the qualified Linux build. USER: owner performs one browser task and one terminal task
through the shell and verifies authority revalidation on return.

### LiteSPM — verdict BLOCKED

**Exists.** The real local daemon contract is captured (JSON-RPC over the local socket,
`daemon.handshake` with `SO_PEERCRED` peer auth, `tools.list`, `host.detect_config`,
22 registered methods) and was observed live on this host against `litespm v0.3.0`.
The Operator routes for connect/install/test deliberately refuse with
`LITESPOM_PACKAGE_SERVICE_NOT_QUALIFIED`, and no `LiteSpmClient` exists in the daemon.

**Remains.** A bounded `LiteSpmClient` (handshake + `tools.list` + `host.detect_config`),
the Discover surface mounted and driven by it with preview-before-mutation, adopt/unmanage
flows that never turn "stop managing" into uninstall, effect recording for every mutation,
and qualification per exact LiteSPM version.

**Evidence to flip.** SYSTEM: observe native MCP entries, preview an adopt, apply it,
verify the config is unchanged outside the previewed scope, then unmanage. USER: owner
performs one install and one unmanage on their own machine.

### Task — verdict BLOCKED

**Exists.** `POST/GET /v1/tasks`, list with filters, `GET /v1/tasks/{id}/planning-readiness`
(always `dispatch_available:false`), `POST /v1/tasks/{id}/spec-revisions` in the READY
window, and the storage tables for revisions/plans/steps/attempts/effects/evidence.
Planner admission fails closed: `start_task_planning_session`/
`activate_task_planning_session` return `TASK_PLANNING_ISOLATION_UNAVAILABLE` before any
write (`storage-sqlite/src/lib.rs:245-251`, `:2493-2661`), and the Effect `STARTED`
transition plus capability-invocation creation are deliberately disabled pending one
atomic admission transaction (`STORAGE.md:895-897`, `:1135-1140`).

**Remains.** Planner session admission under the isolation gate (never an owner bypass),
atomic plan acceptance (`POST /v1/tasks/{id}/plan-revisions`, target-only today), the
single attempt-admission transaction (Effect + invocation + lease + receipts), the
evidence/verification producer, and the task control routes (pause/resume/steer/cancel/
recover/handoff) plus the six-tab detail UI.

**Evidence to flip.** SYSTEM: create → plan → accept → attempt → effect observed →
verification → restart mid-attempt with correct lease fencing. USER: owner runs one task
end to end, including one cancelled and one recovered step.

### Multi-agent — verdict BLOCKED

**Exists.** Delegation and worker profile validation, lead-failover domain logic, and
delegation admission rules in the domain crates.

**Remains.** Worker session start under Attempt scope, host-delegated execution, lead
switch fencing while Attempts run, and verified handoff integration. Worker profiles
cannot be enabled for execution today (`AgentCatalogSettings.tsx:652`).

**Evidence to flip.** SYSTEM: chief delegates one bounded worker Step, the worker runs,
the result integrates and verifies, then a lead switch fences the running Attempt.

### Automation — verdict BLOCKED

**Exists.** Automation CRUD with cron and IANA timezone, manual run and exact-revision
test run, cursor/slot materialization with misfire recording, the newly landed occurrence
settlement from a terminal Task outcome (`settle_started_occurrences_from_task_outcomes`,
guarded per-occurrence, emitting `automation.occurrence.settled.v1` and the local
notification decision), notification preferences and delivery rows. The UI is honest that
trigger execution is not available (`AutomationsPage.tsx:238`).

**Remains.** The recurring trigger-host loop (a daemon poller — today only the served
`POST /v1/automations/host/settle` pass runs on demand), trigger-host cursor retirement on
disable, real OS notification delivery (attempts,
`notification.delivery.changed.v1`, the local OS bridge), sleep/restart recovery, and the
run-history UI linking occurrence → Task → outcome.

**Evidence to flip.** SYSTEM: schedule fires across a sleep/resume and a daemon restart
with no duplicate occurrence and no lost slot; a terminal Task settles its occurrence and
frees the concurrency slot; a cancelled Task skips with a reason. USER: owner sees one
real notification for one real run.

### Memory — verdict BLOCKED

**Exists.** Domain learning/memory rules, Workspace roots, and a Coworker memory policy
editor that is honestly inactive. RAG parsing/retrieval stays behind a qualified provider
boundary with no qualified provider.

**Remains.** A qualified retrieval provider, the scoped memory list with provenance and
conflicts, revoke/purge receipts, and cross-scope leak tests.

**Evidence to flip.** SYSTEM: ingest a scoped source, retrieve it, prove a second scope
cannot see it, revoke it and prove the next retrieval excludes it. USER: owner confirms
the provenance and the revoke receipt.

### Artifacts — verdict BLOCKED

**Exists.** ResourceRef pinning, artifact/version storage, text/Markdown preview, editing,
diff and publish in the UI, upload/download, and the honest "preview unavailable for this
format or size" fallback (`ArtifactWorkbench.tsx:642`).

**Remains.** Per-format generation/preview for PDF, DOCX/XLSX/PPTX and images through
qualified providers, source backlinks, and export that matches the claimed capability.

**Evidence to flip.** SYSTEM: generate one document and one spreadsheet with a real
provider, preview, version, export and reopen. USER: owner edits and reopens their own
file.

### Desktop/release — verdict BLOCKED for V1 GA

**Exists.** Tauri shell with sidebar/drawer focus management, first-run workspace flow,
peer-authenticated IPC, and the `scripts/check.sh` gate.

**Remains.** A packaged installer with signing, backup/restore CLI qualification, a full
accessibility pass, Windows/macOS hardware qualification, systemd install, performance
budgets, and the Settings sections the screen contract requires.

**Evidence to flip.** Per-OS cold-install qualification with exact build hashes, one
end-to-end scenario per screen, and an owner accessibility review.

## Definition of DONE

For every product-area journey: (1) one source-authority command and service; (2) actual route/adapter; (3) state change/readback and idempotency; (4) real qualified external provider/OS; (5) documented negative, crash and cancellation cases; (6) user-visible truth/error/loading states; (7) CODE + SYSTEM + owner USER evidence, exact versions and links.

A green lint/unit suite by itself changes a row to CODE at most. A mock Tauri surface is not SYSTEM. An agent catalog or ACP handshake is not a native session qualification. Unknown provider billing must remain unknown. When a cell cannot run on current hardware mark BLOCKED with the explicit reason; do not silently claim cross-platform support.

## Follow-up implementation runs

Implementation story details and acceptance IDs remain in `backlog.json`, `epics/E*.md`, `reviews/*.md`. This matrix does not silently change their states. Reconcile after real source changes, with all platform/provider versions and named defects in the PR. No calendar release date is inferred.
