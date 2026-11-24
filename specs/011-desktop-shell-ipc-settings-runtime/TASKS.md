# Tasks - Desktop Shell IPC Settings and Runtime Health

**Feature:** 011 | **Status:** ACCEPTED/LOCKED - implementation authorized; tasks not started unless marked.
**Spec revision:** 1.0 | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second)

## Current baseline

Verified in source on 2026-11-09 (branch `docs/audit-competitor-superset`):

- IPC: `crates/operator-ipc/src/windows.rs` (`WindowsEndpoint::derive`, `WindowsEndpoint::bind`, `WindowsIpcListener`, `AuthenticatedWindowsConnection`, eight pending pipe instances) uses `crates/windows-local-security/src/lib.rs` `create_operator_pipe` and `current_logon_sid`. `crates/operator-ipc/src/unix.rs` (`UnixEndpoint`, `UnixIpcListener` with an `authorize_peer` callback over `PeerCredentials`) enforces a 0700 directory and 0600 socket. Limits are constants in `crates/operator-ipc/src/lib.rs` (`MAX_HEADER_BYTES`, `MAX_BODY_BYTES`, `DEFAULT_MAX_IN_FLIGHT_BODY_BYTES`). Tests: unit tests in both transport files, `crates/operator-ipc/tests/unix_endpoint_lifecycle.rs`, `apps/litecoworkd/src/operator_ipc_tests.rs`, `apps/litecoworkd/src/runtime_windows_security_tests.rs` (state-directory DACL, junction and symlink rejection). Not found: a DACL-creation-failure no-fallback test, a remote pipe client rejection test on native Windows, a packaged-peer test.
- Runtime: `apps/litecoworkd/src/runtime.rs` drains only from the process-supervisor signal (`wait_for_shutdown_signal_or_endpoint_failure`, `stop_admission`, `advance_to_stopping_after_operator_drain`) and its comment forbids an Operator stop route until Task/Attempt, Effect, lease and local service-reference settlement is proven. `previewLocalRuntimeStop` and `requestLocalRuntimeStop` exist only as OpenAPI target operations. `apps/litecoworkd/src/main.rs` HELP says there is no authenticated Operator stop command.
- Desktop bridge: `apps/litecowork-ui/src-tauri/src/runtime_lifecycle_bridge.rs` (`get_runtime_lifecycle`, `decode_runtime_lifecycle`) and `apps/litecowork-ui/src-tauri/src/lib.rs` `start_local_runtime`, which uses `systemd_user_service::start`/`verify_active` (`apps/litecowork-ui/src-tauri/src/systemd_user_service.rs`) on Linux and spawns the daemon directly elsewhere.
- Settings: `apps/litecowork-ui/src/settings/SettingsView.tsx` mounts General, Workspaces, Agents (via `AgentCatalogSettings`) and Runtime. Theme, density, text scale and reduced motion are `useState` only. Ctrl + N, Ctrl + W and Ctrl + K are `<kbd>` labels; `App.tsx` keydown handlers cover Escape and Tab focus trapping for the navigation drawer and narrow Workbench only. Runtime shows "Connected (Localhost)". `runtimeLifecycleLoading`, `runtimeLifecycleError` and `runtimeError` props are declared but not rendered. Archive Workspace exists with a two-step confirm (`onArchiveWorkspace`, Operator `archive_workspace`); there is no rename (`updateWorkspacePolicy` accepts only replication fields). `apps/litecowork-ui/tests/settings-accessibility.test.mjs` covers contrast and keyboard access to technical details only.
- Keep-awake: no power-assertion code exists in `apps/` or `crates/`.

Gaps: B-28 native-row qualification, inert-control removal, applied local preferences, Workspace rename, Settings -> Agents served-operation map, keep-awake, stop/update quiescence gate, app-wide keyboard baseline, packaged qualification. The former Usage telemetry task moves to Feature 018 (usage dashboard owner); this package only renders the Usage category unavailable and links there.

## 011-T01 - Promote accepted 011 interfaces into owner docs and schemas

- **Status:** DONE (CODE, contract level): `RuntimeStopPreview` gained settlement lists and `verdict`; `requestLocalRuntimeStop` gained `If-Match`; `PowerAssertion` added as optional `RuntimeLifecycleView.power_assertion`; `renameWorkspace` (PUT /workspaces/{workspaceId}/name) added target-only with `workspace.renamed.v1`; error codes `RUNTIME_STOP_UNSETTLED`, `RUNTIME_QUIESCENCE_UNKNOWN`, `KEEP_AWAKE_UNSUPPORTED`, `WORKSPACE_NAME_INVALID` added; V1 control set pinned in the Settings ledger. No handlers exist; every new operation stays target-only (57 target-only now)
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE
- **Owner:** Runtime, Operator IPC, Workspace owner, Desktop UI (contract owners)
- **Allowed paths:** `docs/RUNTIME-LIFECYCLE.md`, `docs/LOCAL-OPERATOR-IPC.md`, `docs/V1-SETTINGS-CONTRACT-LEDGER.md`, `docs/SEMANTIC-SERVED-UI-MAP.md`, `docs/EVENTS.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/schemas/domain-event.schema.json`, `docs/schemas/error-codes.schema.json`, `apps/litecowork-ui/src/generated/` (regenerated types), this package
- **Contract:** SPEC §5, §7.2; AC-014, AC-015, AC-016, AC-017, AC-019, AC-020
- **Change intent:** In one change: define the `previewLocalRuntimeStop` response (`StopPreview` item lists and verdict) and the `requestLocalRuntimeStop` idempotency/If-Match semantics; add `renameWorkspace` over the existing `workspaces.name` and `Workspace.version`; define the update quiescence gate contract that Feature 019 calls; define the `PowerAssertion` observation and keep-awake modes; pin the exact V1 Settings control set (B-33) in the ledger with each control's owner operation or LOCAL_PREF class; add a Workspace renamed event and the stop/rename/keep-awake error codes. All new operations stay target-only in the route inventory until handlers land.
- **Acceptance:**
  - [x] OpenAPI, event and error-code schemas validate and match owner prose.
  - [x] Ledger lists every control that will be interactive in V1 and nothing else (AC-020).
  - [x] `api:check` passes after regeneration.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `pnpm --dir apps/litecowork-ui run api:check` and `python3 scripts/audit_served_routes.py`
- **Evidence:** schema validation output in CURRENT-RUN.md.
- **Stop:** stop if a promoted field duplicates another owner's schema.

## 011-T02 - Qualify the Windows named-pipe DACL profile (B-28 first row)

- **Status:** DEFERRED (needs a native Windows 11 or 10 x64 runner; WSL2 does not count). Prior partial state kept: PARTIAL (`crates/operator-ipc/src/windows.rs` endpoint/listener and `crates/windows-local-security/src/lib.rs` `create_operator_pipe`, `current_logon_sid` exist with unit tests; missing: DACL-failure no-fallback, remote-client rejection and packaged-peer tests on native Windows)
- **DEFERRED/BLOCKED note:** Why: needs a native Windows runner; WSL2 does not count. Unblocks when: the owner's Windows laptop. Test later at: the owner's Windows laptop.
- **Depends on:** NONE
- **Cross-package prerequisites:** Feature 019 Windows CI runner (native, not WSL2).
- **Owner:** Operator IPC, Security
- **Allowed paths:** `crates/operator-ipc/src/windows.rs`, `crates/operator-ipc/tests/windows_endpoint_lifecycle.rs` (new), `crates/windows-local-security/src/lib.rs`, `apps/litecoworkd/src/runtime_windows_security_tests.rs`, `apps/litecoworkd/src/operator_ipc_tests.rs`
- **Contract:** SPEC §5 B-28, §10; LOCAL-OPERATOR-IPC; AC-001, AC-002, AC-009, AC-010
- **Change intent:** Add a fault-injection seam in `create_operator_pipe` so a DACL failure is testable; assert startup returns a typed error and opens no pipe and no loopback listener. Add tests that a client opening the pipe through remote `\\host\pipe\...` syntax and a client under a different logon SID are rejected, and that the packaged `litecowork-ui` binary connects as the authenticated peer.
- **Acceptance:**
  - [ ] DACL failure: no pipe instance exists afterwards and no TCP listener is bound (AC-009).
  - [ ] Remote and foreign-SID clients are rejected before any frame read (AC-010).
  - [ ] Same-user packaged client is accepted; principal derives from the pipe, never from a header (AC-001).
  - [ ] Frame without the authenticated connection marker is rejected (AC-002).
- **Command:** `cargo test -p operator-ipc --locked windows_endpoint` (filter to be added by this task) and `cargo test -p windows-local-security --locked` and `cargo test -p litecoworkd --locked runtime_windows_security` on native Windows x64
- **Evidence:** native Windows 11 and Windows 10 x64 output in CURRENT-RUN.md (exact build numbers).
- **Stop:** stop if any test ran under WSL2.

## 011-T03 - Qualify the Linux 0600/0700 socket profile (second row)

- **Status:** DEFERRED to the second-user Linux qualification run (needs a second OS user and the packaged build). Prior partial state kept: PARTIAL (`crates/operator-ipc/src/unix.rs` enforces directory/socket modes and peer credentials; `crates/operator-ipc/tests/unix_endpoint_lifecycle.rs` exists; missing: native Linux desktop packaged-peer and foreign-UID evidence)
- **DEFERRED/BLOCKED note:** Why: needs a second OS user and the packaged build. Unblocks when: packaged build from Feature 019. Test later at: native Linux with two OS users.
- **Depends on:** NONE
- **Cross-package prerequisites:** Feature 019 Linux CI runner.
- **Owner:** Operator IPC, Security
- **Allowed paths:** `crates/operator-ipc/src/unix.rs`, `crates/operator-ipc/tests/unix_endpoint_lifecycle.rs`, `apps/litecoworkd/src/operator_ipc_tests.rs`
- **Contract:** SPEC §5 B-28; LOCAL-OPERATOR-IPC; AC-001, AC-002, AC-010
- **Change intent:** Add tests for a foreign-UID peer (run as a second user in CI), a symlinked socket path, a 0755 directory, and the packaged client connecting as the same UID.
- **Acceptance:**
  - [ ] Foreign UID rejected before frame read (AC-010).
  - [ ] Symlink and wrong-mode directory refuse to bind.
  - [ ] Packaged client accepted (AC-001); missing marker rejected (AC-002).
- **Command:** `cargo test -p operator-ipc --locked unix_endpoint` on native Linux x64
- **Evidence:** native Linux x64 output in CURRENT-RUN.md.
- **Stop:** stop if run inside WSL2 or a container without a real second user.

## 011-T04 - IPC admission bounds and frame isolation

- **Status:** DONE (CODE, Linux unix transport; Windows transport shares the same `InFlightBodyBudget` and eight-slot semaphore but the Windows concurrency test is deferred to a native Windows runner, see T02). `crates/operator-ipc/tests/ipc_admission_bounds.rs` added; `connection_admission_is_bounded_and_releases_capacity_after_disconnect` covers the ninth peer.
- **Reference (DeepSeek Harness, MIT, pinned `fcc0ad46`, no code copied yet):** upstream [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159), reuse map [DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md).
  - **P-32 Per-request deadlines with defined meaning-on-timeout for destructive shell-to-daemon calls.** Class: **ADAPT**. Upstream permalinks: [src/host-process.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/host-process.ts), [src/welcome-window.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/welcome-window.ts), [src/update-dialog.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/update-dialog.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 2, 6 and 14 (row P-32). Why: LiteCowork has IPC admission bounds (T04 of package 011) but no stated timeout meaning for stop, update and quit calls. The upstream rule (a timeout on a destructive gate means do not proceed) is reusable; the transport differs (UDS and named pipe, not loopback HTTP). Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React).
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE
- **Owner:** Operator IPC
- **Allowed paths:** `crates/operator-ipc/src/lib.rs`, `crates/operator-ipc/src/async_io.rs`, `crates/operator-ipc/src/unix.rs`, `crates/operator-ipc/src/windows.rs`, `crates/operator-ipc/tests/`
- **Contract:** SPEC §7a IPC limits; AC-013
- **Change intent:** Enforce `DEFAULT_MAX_IN_FLIGHT_BODY_BYTES` across connections with a typed backpressure error; test nine concurrent peers and bodies summing past 128 MiB; assert response frames never interleave.
- **Acceptance:**
  - [x] Ninth concurrent request is held or rejected with the typed error, never accepted (AC-013).
  - [x] Combined bodies over 128 MiB are refused; memory stays bounded (AC-013).
  - [x] Each response correlates to its request_id with no interleaved bytes under 8 concurrent peers.
- **Command:** `cargo test -p operator-ipc --locked ipc_admission_bounds` (filter to be added by this task)
- **Evidence:** test output in CURRENT-RUN.md.
- **Stop:** stop if a bound can be exceeded under any interleaving.

## 011-T05 - Runtime health honesty in the shell

- **Status:** DONE for the UI (CODE): `src/runtime/runtime-health.ts` derives transport, Operator, recovery_state and execution rows from observations; `SettingsView.tsx` renders loading/error states in live regions and no longer says "Localhost"; `tests/settings-runtime-health.test.mjs` added (5 tests). The Tauri-side `cargo test -p litecowork-ui runtime_lifecycle` command was not run in this slice.
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop UI, Runtime
- **Allowed paths:** `apps/litecowork-ui/src/settings/SettingsView.tsx`, `apps/litecowork-ui/src/App.tsx` (runtime status props only), `apps/litecowork-ui/src-tauri/src/runtime_lifecycle_bridge.rs`, `apps/litecowork-ui/tests/runtime-start-readiness.test.ts`, `apps/litecowork-ui/tests/settings-runtime-health.test.mjs` (new)
- **Contract:** SPEC §7a recovery states; RUNTIME-LIFECYCLE; AC-005
- **Change intent:** Render `runtimeLifecycleLoading`, `runtimeLifecycleError` and `runtimeError`; replace "Connected (Localhost)" with the actual transport ("Local IPC (named pipe)" / "Local IPC (socket)"); show recovery_state, Operator readiness and execution readiness as separate rows; DEGRADED keeps Operator commands but shows execution closed.
- **Acceptance:**
  - [x] STOPPED, STARTING and RECOVERING never show execution ready (AC-005).
  - [x] DEGRADED with readiness SERVING shows commands available and execution closed (AC-005).
  - [x] No string "Localhost" remains in Settings.
  - [x] Error and loading states render with accessible live-region announcements.
- **Command:** `pnpm --dir apps/litecowork-ui test` and `cargo test -p litecowork-ui --locked runtime_lifecycle` 
- **Evidence:** test names; IMPLEMENTATION-STATUS Settings row.
- **Stop:** stop if any readiness value is derived from a click rather than an observation.

## 011-T06 - Remove inert Settings controls and gate by owner class (B-29/B-33)

- **Status:** PARTIAL (CODE): `settings-control-map.ts` maps all 19 controls rendered directly by `SettingsView.tsx` (each carries `data-control-id`); `tests/settings-inert-control-audit.test.mjs` (5 tests) fails on an unmapped control, a Target-class control or a missing unavailable entry; shortcut card removed; Usage, Storage and About render as unavailable text with prerequisite. NOT done: the 32 controls inside `AgentCatalogSettings` are outside the map (T09 owns them), the per-category module split with typed context, and the 16-category IA (not a V1 claim, B-33)
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop UI, Product
- **Allowed paths:** `apps/litecowork-ui/src/settings/SettingsView.tsx`, `apps/litecowork-ui/src/settings/settings-view.css`, `apps/litecowork-ui/src/settings/categories/` (new per-category modules), `apps/litecowork-ui/src/settings/settings-control-map.ts` (new), `apps/litecowork-ui/tests/settings-inert-control-audit.test.mjs` (new)
- **Contract:** SPEC §5 B-29, B-33; V1-SETTINGS-CONTRACT-LEDGER; AC-003, AC-007, AC-011, AC-020
- **Change intent:** Add a typed `settings-control-map.ts` with one entry per rendered control (`SettingsControlClass`: LocalOnly, Native, Target, OperatorServed) generated from the T01 ledger. Remove the General appearance controls until T07 wires them, and the shortcut card until T11's registry exists. Render Usage & Monitoring (Feature 018), Storage & Backup (Feature 017) and About & Diagnostics (Feature 018) as unavailable entries with their prerequisite, not as controls. Split categories into modules under `settings/categories/` with a typed context instead of 60 props.
- **Acceptance:**
  - [x] Audit test fails if any SettingsView-rendered input/button lacks a control-map entry with a served operation or applied local effect (AC-020).
  - [x] Target-only entries render as text with a reason and send no mutation (AC-003).
  - [x] Category navigation (keyboard-operable buttons and a labelled list; no new screen-reader run) and unavailable reasons are keyboard and screen-reader accessible (AC-011).
  - [ ] Rich presentation disabled still shows semantic Conversation text (AC-007).
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** audit test output; UI ledger diff.
- **Stop:** stop if a control would be enabled without a ledger entry.

## 011-T07 - Applied and persisted local display preferences

- **Status:** DONE (CODE): `src/settings/local-preferences.ts` validates, persists (webview localStorage under `litecowork.local-preferences.v1`) and applies theme, density, text scale and reduced motion at the document root from `App.tsx`; `tests/local-preferences.test.mjs` (7 tests). Restart survival is proven by reloading from the same storage, not in a packaged app
- **Depends on:** T06
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop UI
- **Allowed paths:** `apps/litecowork-ui/src/settings/local-preferences.ts` (new), `apps/litecowork-ui/src/settings/categories/` , `apps/litecowork-ui/src/App.tsx` (apply at root only), `apps/litecowork-ui/tests/local-preferences.test.ts` (new)
- **Contract:** SPEC §7.2 `LocalPreferenceStore`; DATA-MODEL; AC-004
- **Change intent:** Persist theme, density, text scale and reduced motion in a desktop-local store and apply them at the document root (CSS variables and `prefers-reduced-motion` override). Corrupt or unknown stored values fall back to defaults. No preference touches grants, bindings or Operator state.
- **Acceptance:**
  - [x] Changing a value applies immediately and survives app restart (AC-004).
  - [x] Corrupt stored JSON resets to defaults without crashing.
  - [x] Test proves no Operator call is made by a preference change (AC-004).
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test names; ledger rows switch to LOCAL_PREF served.
- **Stop:** stop if a preference would be synced or replicated.

## 011-T08 - Workspace rename and archive in Settings

- **Status:** DONE for Linux (CODE, SERVED): `WorkspaceService::rename_idempotent` (domain-workspace) over the existing `commit_workspace_idempotent` transaction (no migration), Operator `PUT /v1/workspaces/{workspace_id}/name` (`rename_workspace` in `operator.rs`), `rename_workspace` Tauri command, `workspace-rename.ts` and an inline rename form in Settings. Evidence: `apps/litecoworkd/src/workspace_rename_operator_tests.rs` (5 tests incl. exactly-one-of-four concurrent). NOT verified: the Tauri crate does not compile in this container (gdk-3.0 missing), so the new command is reviewed but uncompiled; no packaged-app run
- **Depends on:** T01, T06
- **Cross-package prerequisites:** NONE
- **Owner:** Workspace owner, Desktop UI
- **Allowed paths:** `crates/storage-sqlite/src/lib.rs` (Workspace mutation next to the existing policy update), `crates/storage-core/src/lib.rs`, `apps/litecoworkd/src/operator.rs` (rename handler and route), `apps/litecowork-ui/src-tauri/src/lib.rs` (rename command), `apps/litecowork-ui/src/settings/categories/`, `apps/litecowork-ui/tests/workspace-rename.test.ts` (new), `crates/storage-sqlite/tests/`
- **Contract:** SPEC §7.2 `renameWorkspace`; AC-017
- **Change intent:** Storage: CAS on `workspaces.version`, trim and bound the name, emit the promoted renamed event and idempotency receipt in one transaction. Operator: authenticated, Workspace-scoped, If-Match and Idempotency-Key required. UI: inline rename with pending, conflict (shows latest name, keeps draft) and archived read-only states; archive keeps its confirm.
- **Acceptance:**
  - [x] Stale If-Match returns conflict with the current name; no write (AC-017).
  - [x] Same key and name returns the original receipt; same key, different name fails.
  - [x] Archived Workspace rejects rename and the UI shows read-only (AC-017).
  - [x] Two concurrent renames: exactly one commits.
- **Command:** `cargo test -p litecoworkd --locked workspace_rename` (tests live in the daemon; no storage-sqlite change was needed) and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output; served-route inventory diff.
- **Stop:** stop if rename would bypass the Workspace version.

## 011-T09 - Settings -> Agents screen over served Feature 002 operations (A-35)

- **Status:** DONE (CODE). Agents heading derives from probed profiles (`src/settings/agents-heading.ts`, 4 tests); descriptor 503 renders UNAVAILABLE via `AgentControlDescriptorPanel`; all 35 interactive elements in `AgentCatalogSettings.tsx` carry `data-control-id` and are classified in `AGENT_CATALOG_CONTROL_MAP` (`settings-control-map.ts`), audited by `tests/agent-catalog-control-map.test.mjs` (4 tests: unique ids, map and render agree, no Target-class control, no sign-in/install/update control); pending (Probing…, disabled probes), receipt (role=status) and error (role=alert) states are pinned by `tests/agent-catalog-action-states.test.mjs` (3 tests). Sign-in and install/update are not rendered as controls (Feature 002 does not serve them).
- **Depends on:** T06
- **Cross-package prerequisites:** Feature 002 OpenCode binding probe fix and AgentControlDescriptor producer tasks.
- **Owner:** Desktop UI (screen), Feature 002 owners (operations)
- **Allowed paths:** `apps/litecowork-ui/src/settings/categories/`, `apps/litecowork-ui/src/agents/AgentCatalogSettings.tsx`, `apps/litecowork-ui/src/settings/settings-control-map.ts`, `apps/litecowork-ui/tests/agent-catalog-settings.test.mjs`
- **Contract:** SPEC §5 A-35, Agents tab; Feature 002 SPEC (operations); AC-019
- **Change intent:** Map every Agents control to its Feature 002 operation and served state. Unserved operations (descriptor, sign-in, install/update) render unavailable with the reason; the screen layout and categories stay owned here and Feature 002 links to it. Remove the heading text that lists agents with no probe.
- **Acceptance:**
  - [x] With descriptor routes returning 503, model/effort controls are shown unavailable, not hidden as ready (AC-019).
  - [x] Probe and binding actions show pending, receipt and error states.
  - [x] Heading names only harnesses with active probes.
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test names; ledger rows.
- **Stop:** stop if this screen would restate Feature 002 operation semantics instead of linking.

## 011-T10 - Notifications entry linking Feature 010 quiet hours

- **Status:** DONE (CODE): Notifications category links to the Notifications page and states that quiet-hours editing is unavailable until Feature 010; no quiet-hours control exists in this package; `settings-inert-control-audit.test.mjs` pins it
- **Depends on:** T06
- **Cross-package prerequisites:** Feature 010 quiet-hours control surface task (A-35).
- **Owner:** Desktop UI
- **Allowed paths:** `apps/litecowork-ui/src/settings/categories/`, `apps/litecowork-ui/src/settings/settings-control-map.ts`, `apps/litecowork-ui/tests/settings-inert-control-audit.test.mjs`
- **Contract:** SPEC Notifications tab; A-35; AC-020
- **Change intent:** Add a Notifications category entry that hosts Feature 010's quiet-hours component when Feature 010 serves it, and otherwise shows an unavailable reason. Shell-local sound/notification choices appear only when the OS notification provider exists.
- **Acceptance:**
  - [x] No quiet-hours control is defined in this package's code (AC-020).
  - [x] Unavailable state names Feature 010 as the prerequisite.
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** audit test output.
- **Stop:** stop if quiet-hours semantics would be duplicated here.

## 011-T11 - App-wide keyboard and focus baseline

- **Status:** PARTIAL (CODE): `src/shell/shortcut-registry.ts` registers Ctrl N and Ctrl K with typed handlers; skip link (`Skip to main content`) targets `<main id="main-content" tabIndex={-1}>`; one global `:focus-visible` outline now covers button, a, textarea, input, select, summary and `[tabindex]`; the Settings General category lists shortcuts generated from the registry (`ShortcutList.tsx`); navigation drawer and Workbench already return focus to their triggers (pinned by source assertions); `tests/keyboard-focus-baseline.test.mjs` (7 tests). Ctrl W stays unregistered (WebView/OS reserved). NOT done: the keyboard-only browser path (New Chat, composer, Workbench, Settings, close drawer) and a real Tab-order run need Chrome (`pnpm run test:browser`); none is installed in this container, so AC-018 keyboard path and AC-011 announcement checks remain unmeasured.
- **Reference (DeepSeek Harness, MIT, pinned `fcc0ad46`, no code copied yet):** upstream [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159), reuse map [DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md).
  - **P-46 IME-safe shortcut dispatch rules (no match during composition, dead keys or AltGr; reset on blur; balanced keyups).** Class: **ADAPT (rules only)**. Upstream permalinks: [src/keyboard.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/keyboard.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Section 14 (row P-46). Why: `src/keyboard.ts` imports `electron` and four workspace packages, so it is not reusable as a file; the rules become acceptance cases for the existing `src/shell/shortcut-registry.ts`. Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React).
- **Depends on:** T06
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop UI
- **Allowed paths:** `apps/litecowork-ui/src/App.tsx` (keyboard wiring only), `apps/litecowork-ui/src/shell/shortcut-registry.ts` (new), `apps/litecowork-ui/src/settings/categories/`, `apps/litecowork-ui/tests/keyboard-focus-baseline.test.mjs` (new), `apps/litecowork-ui/tests/browser/`
- **Contract:** SPEC §13 accessibility; V1-UI-SCREEN-CONTRACT; AC-011, AC-018
- **Change intent:** One shortcut registry that registers handlers and generates the Settings shortcut list and the sidebar label ("Ctrl N" in `App.tsx`). Visible focus style on all interactive elements; focus return after dialogs/drawers; skip link to main content; Tab order through navigation, composer, Workbench and Settings.
- **Acceptance:**
  - [x] Every listed shortcut has a registered handler; test fails on a label without a handler (AC-018).
  - [ ] Keyboard-only path: open New Chat, send focus to composer, open Workbench, open Settings, change category, close drawer, focus returns (AC-018).
  - [ ] Settings categories reachable and announced (AC-011).
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run test:browser`
- **Evidence:** test output; a11y section of IMPLEMENTATION-STATUS.
- **Stop:** stop if a shortcut conflicts with an OS or WebView reserved key without a documented alternative.

## 011-T12 - Keep-awake power assertion

- **Status:** PARTIAL (CODE, not wired). The prerequisite definition of "work runs" now exists (admitted Conversation turns and non-terminal Attempts from `RuntimeLifecycleStore::read_runtime_stop_observation`, T13 prerequisite). DONE: `apps/litecoworkd/src/power_assertion.rs` state machine (HELD only after the backend confirms; an OS refusal or failed release is UNKNOWN; selecting a mode never reports HELD; `WHILE_WORK_RUNS_ON_POWER` is UNSUPPORTED and unselectable when the power source cannot be read; an unavailable provider is UNSUPPORTED; drain clears the mode and releases before STOPPING) with 9 unit tests (8 over a fake backend, 1 over the real Linux `systemd-inhibit` backend which accepts Held or Unknown), and the Linux `systemd-inhibit` child backend. NOT done: wiring the poll and drain hook into `runtime.rs`, publishing `RuntimeLifecycleView.power_assertion`, the Settings Advanced UI and `tests/keep-awake.test.mjs`, the Windows `PowerCreateRequest` backend (native Windows runner), and any `systemd-inhibit --list` capture (this container has no logind, so HELD was never observed on a real OS)
- **Reference (DeepSeek Harness, MIT, pinned `fcc0ad46`, no code copied yet):** upstream [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159), reuse map [DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md).
  - **P-31 QuiescenceReport (one "interruptible work" predicate; unknown means busy; lock then learn; inspect once).** Class: **ADAPT**. Upstream permalinks: [src/host-process.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/host-process.ts), [src/quit-confirmation.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/quit-confirmation.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 3 and 14 (row P-31). Why: LiteCowork describes the same predicate in three places (T12, T13, T14 of package 011). Upstream shows the same facts must back stop, update, quit and keep-awake, and that unknown resolves to busy. The inputs differ (admitted turns, admitted Attempts, Terminal and Browser sessions, armed Automations, pending approvals, not Electron tasks), so it is an ADAPT, not a port. Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React).
- **Depends on:** T01
- **Cross-package prerequisites:** Feature 001 admitted-turn state and Feature 005 admitted-Attempt state (definition of "work runs").
- **Owner:** Runtime, native desktop bridge
- **Allowed paths:** `apps/litecoworkd/src/power_assertion.rs` (new), `apps/litecoworkd/src/runtime.rs` (hold/release hooks), `crates/windows-local-security/src/lib.rs` (Power request wrappers), `crates/windows-local-security/Cargo.toml`, `apps/litecowork-ui/src/settings/categories/`, `apps/litecowork-ui/tests/keep-awake.test.mjs` (new)
- **Contract:** SPEC Advanced tab, §7.2 `PowerAssertion`; AC-016
- **Change intent:** Daemon holds the assertion while admitted work runs: Windows `PowerCreateRequest`/`PowerSetRequest` via `windows-sys`; Linux logind `Inhibit` (or `systemd-inhibit`). Battery-only mode is UNSUPPORTED where the OS cannot report power source. Release on drain, stop and when work ends; clear at restart. UI shows the observed state only.
- **Acceptance:**
  - [ ] HELD appears only after the OS call succeeds; OS refusal shows UNKNOWN (AC-016).
  - [ ] Drain releases the assertion before STOPPING (AC-016).
  - [ ] Unsupported mode renders UNSUPPORTED and cannot be selected (AC-016).
- **Command:** `cargo test -p litecoworkd --locked power_assertion` (filter to be added by this task) on each native row and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** per-row output with `powercfg /requests` (Windows) or `systemd-inhibit --list` (Linux) capture in CURRENT-RUN.md.
- **Stop:** stop if the UI could show HELD from the radio selection alone.

## 011-T13 - Authenticated Operator stop with settlement proof

- **Status:** PARTIAL (prerequisite half DONE, request half BLOCKED). DONE (CODE, SERVED, Linux): the read-only `previewLocalRuntimeStop` (`GET /v1/runtime-lifecycle/stop-preview`) over `RuntimeLifecycleStore::read_runtime_stop_observation` (admitted Conversation turns, non-terminal Attempts, unresolved Effects STARTED/ACKNOWLEDGED/RECONCILING/AMBIGUOUS, ACTIVE/RELEASING execution and environment control leases, pending workspace-root cleanup), `apps/litecoworkd/src/runtime_stop_operator.rs` (verdict BLOCKED when any durable list is non-empty, UNKNOWN when a live-work observer is absent, QUIESCENT only with a positive observer; 5 unit tests), an IPC route test, a storage test, OpenAPI description, inventory 175/1/55. The verdict is never QUIESCENT in this build because no owner hook proves absence of live agent sessions, child processes and terminal or browser sessions (Feature 001 effective Stop with observed quiescence, Feature 005 settlement, Feature 008 sessions). STILL BLOCKED: `requestLocalRuntimeStop` (admission fence, DRAINING, STOPPED with positive quiescence, idempotent receipt, If-Match, authentication test for the mutation) stays unmounted; `runtime.rs` still forbids the route
- **Reference (DeepSeek Harness, MIT, pinned `fcc0ad46`, no code copied yet):** upstream [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159), reuse map [DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md).
  - **P-31 QuiescenceReport (one "interruptible work" predicate; unknown means busy; lock then learn; inspect once).** Class: **ADAPT**. Upstream permalinks: [src/host-process.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/host-process.ts), [src/quit-confirmation.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/quit-confirmation.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 3 and 14 (row P-31). Why: LiteCowork describes the same predicate in three places (T12, T13, T14 of package 011). Upstream shows the same facts must back stop, update, quit and keep-awake, and that unknown resolves to busy. The inputs differ (admitted turns, admitted Attempts, Terminal and Browser sessions, armed Automations, pending approvals, not Electron tasks), so it is an ADAPT, not a port. Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React).
  - **P-32 Per-request deadlines with defined meaning-on-timeout for destructive shell-to-daemon calls.** Class: **ADAPT**. Upstream permalinks: [src/host-process.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/host-process.ts), [src/welcome-window.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/welcome-window.ts), [src/update-dialog.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/update-dialog.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 2, 6 and 14 (row P-32). Why: LiteCowork has IPC admission bounds (T04 of package 011) but no stated timeout meaning for stop, update and quit calls. The upstream rule (a timeout on a destructive gate means do not proceed) is reusable; the transport differs (UDS and named pipe, not loopback HTTP). Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React).
- **Depends on:** T01, T04
- **Cross-package prerequisites:** Feature 001 Stop/quiescence task (A-25); Feature 005 Effect reconciliation and lease release tasks; Feature 008 terminal and browser session settlement (when those exist).
- **Owner:** Runtime
- **Allowed paths:** `apps/litecoworkd/src/runtime.rs`, `apps/litecoworkd/src/runtime_stop_operator.rs` (new), `apps/litecoworkd/src/operator.rs` (route mount only), `apps/litecoworkd/src/runtime_shutdown_tests.rs`, `apps/litecoworkd/src/main.rs` (HELP text)
- **Contract:** SPEC §7.2 stop preview; RUNTIME-LIFECYCLE drain; ACCEPTANCE-AND-E2E-PLAN "Desktop stop/update qualification gate"; AC-012, AC-014
- **Change intent:** Implement `previewLocalRuntimeStop` as a read over owner records and `requestLocalRuntimeStop` that fences admission (`stop_admission`), persists DRAINING, waits for each owner's settlement, and reaches STOPPED only with positive quiescence; otherwise stays DRAINING with UNKNOWN items. Route mounts only when every owner settlement hook exists. Update HELP to describe the route.
- **Acceptance:**
  - [ ] Idle stop reaches STOPPED with a QUIESCENT verdict (AC-014).
  - [ ] Active turn, dispatched unresolved Effect, outstanding ExecutionLease/EnvironmentControlLease, child process that outlives its parent and pending cleanup each block STOPPED and are listed (AC-014).
  - [ ] Admission racing drain: new requests after the fence are rejected (AC-012).
  - [ ] Duplicate stop request returns the same receipt; stale If-Match fails.
  - [ ] Unauthenticated stop is rejected.
- **Command:** `cargo test -p litecoworkd --locked runtime_shutdown` and `cargo test -p litecoworkd --locked runtime_stop_gate` (filter to be added by this task)
- **Evidence:** gate journey matrix in CURRENT-RUN.md.
- **Stop:** stop if any owner lacks a settlement hook; keep the route unmounted.

## 011-T14 - Desktop stop and update UI with quiescence gate

- **Status:** PARTIAL (CODE; Tauri bridge not compiled here). DONE: `src/runtime/runtime-stop-state.ts` (pure summary: Stop safely disabled with reasons for BLOCKED/UNKNOWN and always while the stop command is unmounted, update gate allowed only on QUIESCENT for the exact incarnation, `runtimeStoppedLabel` shows "Runtime stopped" only from an observed STOPPED state so window close never does), `RuntimeStopPreviewPanel.tsx`, a mapped `runtime-stop-preview` control in Settings > Runtime wired in `App.tsx` to `get_runtime_stop_preview`, `tests/runtime-stop-update.test.mjs` (7 tests). Rust bridge `get_runtime_stop_preview` and `check_update_quiescence` in `runtime_lifecycle_bridge.rs` with a fail-closed decoder (contradictory QUIESCENT, foreign identity, unknown fields and unknown blocker codes rejected); the decoder and its 7 unit tests were compiled and run in an isolated scratch crate (7 pass), but `src-tauri` itself cannot build here (gdk-3.0), so the `#[tauri::command]` wrappers and handler registration in `lib.rs` now compile as part of the Tauri crate on the connected Windows host (Feature 002 focused bridge tests, 2026-11-11); the runtime_stop tests and packaged user path remain unverified. NOT done: the actual Stop request flow (needs T13 request half), the install-update refusal inside `install_update` (Feature 019 T11, B-43), cargo test of `runtime_stop` on a host that can build src-tauri.
- **Reference (DeepSeek Harness, MIT, pinned `fcc0ad46`, no code copied yet):** upstream [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159), reuse map [DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md).
  - **P-31 QuiescenceReport (one "interruptible work" predicate; unknown means busy; lock then learn; inspect once).** Class: **ADAPT**. Upstream permalinks: [src/host-process.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/host-process.ts), [src/quit-confirmation.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/quit-confirmation.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 3 and 14 (row P-31). Why: LiteCowork describes the same predicate in three places (T12, T13, T14 of package 011). Upstream shows the same facts must back stop, update, quit and keep-awake, and that unknown resolves to busy. The inputs differ (admitted turns, admitted Attempts, Terminal and Browser sessions, armed Automations, pending approvals, not Electron tasks), so it is an ADAPT, not a port. Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React).
  - **P-34 Updater coordinator spec (generation token, single-flight verbs, no auto-install, backoff ladder, refuse after unclean exit).** Class: **ADAPT (update-schedule arithmetic is an ADOPT candidate)**. Upstream permalinks: [src/update-coordinator.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/update-coordinator.ts), [src/update-schedule.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/update-schedule.ts), [src/update-presentation.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/update-presentation.ts), [src/update-attention.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/update-attention.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 10 and 14 (row P-34). Why: The LiteCowork UI reducer already exists (`apps/litecowork-ui/src/updates/`, 16 tests). Upstream adds the daemon-side rules: generation token, graceful-stop ladder with refuse-on-unclean-exit, 14-way failure taxonomy. `update-schedule.ts` is pure arithmetic over a duration helper and coordinator types, so only the arithmetic is a port candidate; the coordinator is an ADAPT because the Tauri updater differs from electron-updater. Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React). For the schedule arithmetic only: If any text or structure is carried over, MIT applies (upstream LICENSE, "Copyright (c) 2026 DeepSeek"): keep that copyright line and the MIT permission notice in the third-party notices produced by task T13 of package 019, and cite the upstream path and SHA `fcc0ad46` in the code comment.
- **Depends on:** T13
- **Cross-package prerequisites:** Feature 019 updater (calls the gate).
- **Owner:** Desktop UI, Runtime
- **Allowed paths:** `apps/litecowork-ui/src-tauri/src/runtime_lifecycle_bridge.rs`, `apps/litecowork-ui/src-tauri/src/lib.rs` (commands), `apps/litecowork-ui/src/settings/categories/`, `apps/litecowork-ui/tests/runtime-stop-update.test.mjs` (new)
- **Contract:** SPEC Advanced tab, §7.2 `UpdateQuiescenceGate`; AC-014, AC-015
- **Change intent:** Tauri commands for preview and request stop and for `check_update_quiescence`. UI: Preview stop lists running and unknown items; Stop safely enabled only on QUIESCENT; update/install flow refuses without positive quiescence and offers Retry.
- **Acceptance:**
  - [x] Stop button disabled with reasons while preview is BLOCKED or UNKNOWN (AC-014).
  - [x] Update refused without positive quiescence; message lists what is running (AC-015). (UI gate and bridge decoder only; no installer path exists yet.)
  - [x] Window close never shows "Runtime stopped".
- **Command:** `cargo test -p litecowork-ui --locked runtime_stop` (filter to be added by this task) and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output; ledger rows.
- **Stop:** stop if any path installs an update without the gate.

## 011-T15 - Shell failure and recovery

- **Status:** PARTIAL (`apps/litecoworkd/src/runtime_recovery_tests.rs`, `runtime_shutdown_tests.rs`, `runtime_process_tests.rs` cover daemon-side recovery and drain; missing: desktop-side disconnected state, window-close vs Runtime state, simultaneous stop/update)
- **Reference (DeepSeek Harness, MIT, pinned `fcc0ad46`, no code copied yet):** upstream [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159), reuse map [DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md).
  - **P-33 Bounded rotating JSONL journal with partial-line repair, disable-on-uncertain-write, and an isolated support-bundle collector.** Class: **ADOPT (algorithm port)**. Upstream permalinks: [src/operation-journal.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/operation-journal.ts), [src/operation-history.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/operation-history.ts), [src/operation-report-job.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/operation-report-job.ts), [src/crash-report.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/crash-report.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 8 and 14 (row P-33). Why: `operation-journal.ts` imports only `node:fs/promises` and `node:path`, so the algorithm (two segments, record cap, queue cap, partial-line repair, bounded drain) ports to Rust directly. It fits the rule of no unbounded logs (T07 of package 018) and records must stay redacted at the producer. Reuse: Algorithm only, reimplement in Rust (or in the LiteCowork TypeScript tree for UI logic); the upstream source has only Node imports so the algorithm ports directly, but the file itself is not imported. If any text or structure is carried over, MIT applies (upstream LICENSE, "Copyright (c) 2026 DeepSeek"): keep that copyright line and the MIT permission notice in the third-party notices produced by task T13 of package 019, and cite the upstream path and SHA `fcc0ad46` in the code comment.
- **Depends on:** T05, T14
- **Cross-package prerequisites:** NONE
- **Owner:** Runtime, Desktop UI
- **Allowed paths:** `apps/litecoworkd/src/runtime_recovery_tests.rs`, `apps/litecoworkd/src/runtime_shutdown_tests.rs`, `apps/litecowork-ui/src-tauri/src/lib.rs`, `apps/litecowork-ui/tests/browser/`
- **Contract:** SPEC §11, §12; AC-005, AC-006, AC-012, AC-015
- **Change intent:** Tests and fixes for: daemon kill while UI open shows disconnected (not an empty Workspace); restart produces a new incarnation and the UI drops stale identity; window close with the Runtime running keeps it running and reports it; drain deadline handling; simultaneous stop and update; handler deadline (504) treated as unsettled.
- **Acceptance:**
  - [ ] Daemon crash: UI shows disconnected within the readiness poll interval (AC-005).
  - [ ] Window close: Runtime stays running and is reported running on reopen (AC-006).
  - [ ] Drain never claims STOPPED before admitted handlers finish or time out unsettled (AC-012).
  - [ ] Concurrent stop and update produce one drain and a refused update (AC-015).
- **Command:** `cargo test -p litecoworkd --locked runtime_recovery` and `cargo test -p litecoworkd --locked runtime_shutdown` and `pnpm --dir apps/litecowork-ui run test:browser`
- **Evidence:** test output in CURRENT-RUN.md.
- **Stop:** stop if any recovery path infers success from intent.

## 011-T16 - Cross-layer traces for shell and Settings operations

- **Status:** PARTIAL. The trace table is in `contracts/ui-behavior.md` ("Cross-layer traces (T16)") with symbols checked against source on 2026-11-10; it lists three gaps instead of documenting them as present (keep-awake wiring and UI, Stop safely, update check and install commands). The Settings controls of SettingsView and AgentCatalogSettings each map to a Tauri command or a local effect. Not done: WIRING.yaml edge updates and `docs/SEMANTIC-SERVED-UI-MAP.md` rows for the new stop-preview and dashboard sources
- **Depends on:** T07-T12, T14
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop UI, Runtime, Operator IPC
- **Allowed paths:** this package (`contracts/ui-behavior.md`, `WIRING.yaml`), `docs/SEMANTIC-SERVED-UI-MAP.md`
- **Contract:** AGENTS.md §4
- **Change intent:** Record the full trace for: start Runtime, refresh health, rename Workspace, archive Workspace, change a local preference, keep-awake, preview stop, stop safely, update check, each Agents action, and the Notifications link.
- **Acceptance:**
  - [ ] Every interactive Settings control has a complete trace with real symbol names.
  - [ ] No trace has a control without a route or a route without a control.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** trace table in contracts/ui-behavior.md.
- **Stop:** stop if a trace names a missing symbol.

## 011-T17 - Packaged qualification on Windows x64 then Linux x64

- **Status:** DEFERRED (packaged qualification needs native Windows x64 then Linux x64 hosts and the packaged installers from Feature 019). NOT STARTED
- **DEFERRED/BLOCKED note:** Why: packaged qualification needs installers from Feature 019 and native hosts. Unblocks when: Feature 019 packaging. Test later at: the owner's Windows laptop, then native Linux x64.
- **Depends on:** T02, T03, T15, T16
- **Cross-package prerequisites:** Feature 019 release matrix, installer and signing; Feature 001 qualified OpenCode v2.0.32 route (E2E-01 shell portion).
- **Owner:** QA, Product (owner USER evidence)
- **Allowed paths:** `apps/litecowork-ui/tests/browser/`, `scripts/v1_platform_matrix.py` (row entries only)
- **Contract:** docs/ACCEPTANCE-AND-E2E-PLAN.md E2E-01 (shell and IPC parts), E2E-11 (fallback, AC-007), "Desktop stop/update qualification gate"; AC-001 through AC-020
- **Change intent:** On the packaged desktop: fresh install, authenticated IPC, Settings per-category journeys, keyboard-only pass, keep-awake, rename/archive, stop/update gate journey (idle, active turn, unresolved Effect, leases, outliving child, pending cleanup, simultaneous stop/update, admission racing drain, crash/timeout, restart). Windows row first, Linux second.
- **Acceptance:**
  - [ ] CODE, SYSTEM and owner USER evidence per row with exact OS build and app build (AC-008: no WSL2).
  - [ ] Every denial and recovery case in the gate journey passes (AC-014, AC-015).
  - [ ] Keyboard-only and screen-reader passes recorded (AC-011, AC-018).
- **Command:** `pnpm --dir apps/litecowork-ui run test:browser` and `python3 scripts/v1_platform_matrix.py`
- **Evidence:** evidence bundle per row in CURRENT-RUN.md and docs/IMPLEMENTATION-STATUS.md.
- **Stop:** stop if any row lacks owner USER evidence.

## 011-T18 - Documentation sync

- **Status:** PARTIAL: API.md, RUNTIME-LIFECYCLE.md, V1-SETTINGS-CONTRACT-LEDGER (generated), SEMANTIC-SERVED-UI-MAP, FEATURE-AND-UI-CONTRACT-COVERAGE, the contradiction register A-03 and IMPLEMENTATION-STATUS were updated for the served stop preview and 175/1/55; the ui-behavior trace table was added. NOT done: the V1 UI action-state ledger rows for Dashboard, More navigation and the update gate, and the full status-document sweep for the control counts
- **Depends on:** T17
- **Cross-package prerequisites:** NONE
- **Owner:** Documentation owner (orchestrator)
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md`, `docs/schemas/served-route-inventory.json`, `docs/SETTINGS-AND-SHELL-SURFACE-AUDIT.md`, `docs/V1-SETTINGS-CONTRACT-LEDGER.md`, `docs/SEMANTIC-SERVED-UI-MAP.md`, `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md`, this package
- **Contract:** AGENTS.md §1, §4
- **Change intent:** Regenerate the route inventory; update the Settings audit, ledger served flags and IMPLEMENTATION-STATUS rows; close register rows for B-28, B-29, B-33 and A-35 obligations that are met; refresh SPEC §4 crosswalk.
- **Acceptance:**
  - [ ] Inventory matches mounted routes, including the stop route only if mounted.
  - [ ] No document still claims "Connected (Localhost)" or inert controls as current.
- **Command:** `python3 scripts/audit_served_routes.py` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** documentation diff.
- **Stop:** stop if a status claim exceeds recorded evidence.

## 011-T19 - Shell reachability and truthfulness (P-05)

- **Status:** PARTIAL (CODE; rendered browser journey passed; cross-package and packaged gates remain). **USER-DELEGATED DIRECTION (A-121/A-123):** the shell now exposes New Chat, Recent Chats, Projects, Needs You, More and footer Settings. Projects is a directory over the existing authenticated Workspace projection, not a new Project aggregate. Selecting a Workspace uses the existing guarded page-navigation path, changes the active Workspace, clears the selected Conversation, and opens a fresh blank Conversation without creating or sending anything. Recent Chats shows only the selected Workspace's Conversation projection and distinguishes IDLE, LOADING, READY-empty, READY, FAILED and UNAVAILABLE; cached chats are hidden without a current Workspace/Runtime. Every other mounted page stays reachable under More; no composite Needs You count is invented. A dirty-Library route request closes the narrow drawer without shifting focus into inert content; Stay returns focus to the current screen's Menu trigger, while explicit Save/Discard completes the pending route with the drawer closed. This remains implementation direction only; the accepted Feature 011 package and V1 release navigation contract are not formally amended.
- **Reference (DeepSeek Harness, MIT, pinned `fcc0ad46`, no code copied yet):** upstream [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159), reuse map [DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md).
  - **P-37 Golden files for user-visible dialog copy and golden scenario lists for installer tests.** Class: **ADAPT**. Upstream permalinks: [tests/expected/](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/tests/expected), [tests/expected/windows-installer.json](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/tests/expected/windows-installer.json), [tests/expected/windows-uninstall.json](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/tests/expected/windows-uninstall.json), [src/locale.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/locale.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 12 and 14 (row P-37). Why: A reviewable golden for stop, update, recovery and failure dialog copy is a strong guard for the truthfulness rules (T19 of package 011). The files themselves hold DeepSeek strings and are not reusable; only the technique is. Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React). If a scenario list from `windows-uninstall.json` or `windows-installer.json` is copied verbatim as a test outline, if any text or structure is carried over, MIT applies (upstream LICENSE, "Copyright (c) 2026 DeepSeek"): keep that copyright line and the MIT permission notice in the third-party notices produced by task T13 of package 019, and cite the upstream path and SHA `fcc0ad46` in the code comment.
- **Depends on:** T06
- **Cross-package prerequisites:** Feature 010 local-poll notification delivery for the Needs You count source; Feature 014 Recent Chats read path.
- **Owner:** Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/App.tsx`, `apps/litecowork-ui/src/`, `apps/litecowork-ui/tests/`, `apps/litecowork-ui/scripts/check-rendered-ui.mjs` (rendered journey coverage and startup readiness only), adjacent tests.
- **Contract:** SPEC §12, §14; AC-021; V1-UI-SCREEN-CONTRACT; register A-35.
- **Change intent:** make the shell compact, reachable and truthful under the user-delegated mapping in A-123. Keep New Chat and Recent Chats visible, add a Projects directory over the existing Workspace list (no Project entity or new authorization boundary), expose Needs You directly, move less-frequent domains under More, and keep Settings in the footer. Selection only changes the active Workspace context and follows existing dirty/focus guards. Distinguish loading, empty, unavailable and error states using the exact source projection; never invent Workspace/chat/task/artifact counts or a composite Needs You count. Keep every mounted route reachable and remove any navigation control that only simulates behavior.
- **Acceptance:**
  - [x] Every mounted page has a navigation path, or is removed from V1 with a recorded owner decision (AC-021).
  - [x] Back/Forward are absent rather than pretending to navigate history (AC-021).
  - [x] Primary shell renders New Chat, Recent Chats, Projects, Needs You, More and Settings; remaining mounted pages stay reachable under More (A-123; source/component CODE tests).
  - [x] Projects renders only existing Workspace projection rows and distinguishes loading, ready-empty, runtime-unavailable and list-error states (A-123; component CODE tests).
  - [x] A missing or failed source renders unavailable/error, not empty; Recent Chats separates IDLE, LOADING, READY-empty/rows, FAILED and UNAVAILABLE and hides cached rows without current Workspace/Runtime (A-123; component CODE tests).
  - [x] No Needs You count or cross-Workspace count is fabricated (A-123; source/component CODE tests).
  - [x] Mounted Projects selection opens a fresh blank Conversation after the Library guard and produces no Conversation create/send, Task, Runtime, provider or grant side effect (rendered browser journey; guarded command filter).
  - [x] The narrow Conversation header exposes a Menu trigger that opens the shared navigation drawer, focuses its close control, reports expanded state, and returns focus to the current screen trigger on Escape or route selection (AC-018, M17; rendered browser journey).
  - [x] Keyboard navigation, current-route disclosure and focus restoration work in the rendered shell, including narrow navigation and dirty Library state (AC-018, M17; rendered browser journey).
- **Command:** focused UI tests and `LITECOWORK_RENDERED_CHECK_GROUP=shell-reachability pnpm --dir apps/litecowork-ui run test:browser`.
- **Evidence:** the focused shell, Conversation and source/component tests pass **56/56**; `pnpm exec tsc --noEmit`, `pnpm run api:check` and `node --check scripts/check-rendered-ui.mjs` pass. On 2026-11-24, `LITECOWORK_RENDERED_CHECK_GROUP=shell-reachability pnpm run test:browser` passed all four checks and `LITECOWORK_RENDERED_CHECK_GROUP=conversation pnpm run test:browser` passed all 15 checks in Chrome 158.0.8127.38 against the production-built rendered harness. The launcher now discovers common Windows Chrome/Edge paths when `LITECOWORK_CHROME` is unset; no application code or test assertions were bypassed. The shell journey covers Workspace-backed Projects, More, narrow-drawer keyboard/focus, and dirty-Library Stay/Discard. The Conversation journey covers truthful blank/draft behavior, Runtime-unready send gating, transient streamed output/recovery, shell navigation and RichPresentation invalidation/retry. Documentation and contract validators are rerun for this handoff and their exact current results are in CURRENT-RUN.md. This is rendered browser CODE evidence only: it is not a packaged Tauri journey, Feature 010/014 completion, package acceptance, INTEGRATED or QUALIFIED evidence. The full UI suite remains red at its latest 658/9/5 baseline and was not rerun for this slice.
- **Stop:** a page cannot be given a real navigation path; remove it from V1 rather than leave it unreachable.

## 011-T20 - Decompose App.tsx and the Rust god files behind existing boundaries (P-07 and P-19)

- **Status:** PARTIAL (UI slice only; no behavior change; all UI tests unchanged except two source-string pins updated in earlier commits). DONE: one shared `src/shell/OperatorUnavailablePanel.tsx` replaces the six duplicated "Local Operator unavailable" page bodies (Coworkers, Notifications, Ideas, Routines, Discover, Automations); `LibraryPage` (752 lines) moved to `src/library/LibraryPage.tsx` with its helpers in `src/library/library-format.ts`; App.tsx went from 4,619 to 3,585 lines together with the SettingsPage deletion (T21). Guarded by two tests in `tests/shell-reachability.test.mjs`. NOT done: the HomePage and WorkPage extractions, the shell/ module, and the Rust decompositions of `operator.rs` (9.7k lines), `crates/storage-sqlite/src/lib.rs` (24k lines) and `src-tauri/src/lib.rs`: these are large mechanical moves whose verification needs the full Rust test suites and the src-tauri build (not possible here), so they were deliberately not started
- **Depends on:** T06
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop UI, Agent Runtime, Storage.
- **Allowed paths:** `apps/litecowork-ui/src/shell/` (new), `apps/litecowork-ui/src/`, `apps/litecoworkd/src/operator.rs`, `crates/storage-sqlite/src/lib.rs`, `apps/litecowork-ui/src-tauri/src/lib.rs`, adjacent tests.
- **Contract:** SPEC §12; AC-022; ARCHITECTURE boundaries; audit section 6.
- **Change intent:** split `App.tsx` into a `shell/` plus one module per page (LibraryPage, HomePage, WorkPage first) with one shared "Operator unavailable" panel replacing the six duplicated copies; and decompose the god files `operator.rs`, `crates/storage-sqlite/src/lib.rs` and `apps/litecowork-ui/src-tauri/src/lib.rs` into owned modules behind the existing boundaries (for example an `ipc_server.rs` plus a router module, and the existing `*_bridge.rs` pattern). No behavior change; existing tests pass unchanged.
- **Acceptance:**
  - [ ] App.tsx is split into a shell plus per-page modules with one shared Operator-unavailable panel; existing UI tests pass unchanged (AC-022).
  - [ ] operator.rs, storage-sqlite/src/lib.rs and src-tauri/src/lib.rs are decomposed behind existing boundaries with no behavior change and existing cargo tests pass (AC-022).
  - [ ] No new authority, route or record is introduced by the split.
- **Command:** `pnpm --dir apps/litecowork-ui test` and `cargo test -p litecoworkd --locked` and `cargo test -p storage-sqlite --locked`
- **Evidence:** before/after module listing and green test output in Completion evidence.
- **Stop:** a split would change behavior or move authority; keep the boundary and record the limit.

## 011-T21 - Merge SettingsPage into SettingsView with precedence notes (P-08)

- **Status:** DONE (CODE). `SettingsPage`, `SettingsPageProps` and `localServiceCopy` are deleted from `App.tsx`; `SettingsView` is the one Settings authority (guarded by `tests/settings-merge.test.mjs`, 4 tests). Merged into `SettingsView`: instruction-conflict recovery ("Use this version", `workspaces-instructions-use-latest`), revision history with source provenance, and the Runtime enrollment card (`runtime-enroll`, `runtime-enrollment-refresh`), all mapped in `settings-control-map.ts`; the archive checklist already existed there. Precedence notes (`data-precedence-note`) on Workspace instructions and the default lead agent. The `navigation` array is not unused any more: the sidebar More group renders from it (see T19), so it was kept instead of deleted.
- **Depends on:** T06, T20
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/settings/`, `apps/litecowork-ui/src/App.tsx`, adjacent tests.
- **Contract:** SPEC §12; AC-023; register A-35; V1-SETTINGS-CONTRACT-LEDGER.
- **Change intent:** merge the unmounted `SettingsPage` features (instruction-conflict recovery "Use this version", revision history, the Runtime enrollment card, the archive checklist) into the mounted `SettingsView`, delete `SettingsPage` and the unused `navigation` array, and add a precedence note on each surface where a setting has a Coworker or Automation override. Cross-reference register A-35 for screen ownership; do not restate the ownership rule here.
- **Acceptance:**
  - [x] One Settings authority remains; conflict recovery and Runtime enrollment are reachable; `SettingsPage` and the unused `navigation` array are deleted (AC-023).
  - [x] Each setting with a Coworker or Automation override shows a precedence note (AC-023).
  - [x] Negative: no second settings surface or duplicate navigation array remains in the source.
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** merged-view test output in Completion evidence.
- **Stop:** a merged feature has no served owner operation; render it unavailable rather than inert.

## 011-T22 - Persistent Workspace Dashboard with pinnable widgets (P-16)

- **Status:** PARTIAL (CODE). `src/dashboard/dashboard-state.ts` (pure: per-Workspace pins in localStorage `litecowork.dashboard.pins.v1`, validated, corrupt state falls back to all pinned, bounded workspace entries, count summaries that flag first-page truncation), `WorkspaceDashboard.tsx` (pin/unpin with `aria-pressed`, UNAVAILABLE with reason, errors become UNAVAILABLE, stale responses ignored on Workspace switch), a new `Dashboard` Page in the sidebar More group, `tests/workspace-dashboard.test.mjs` (8 tests). Sources: Task status reads `list_tasks` (first 50, counted by status), Automation status reads `automationApi.list`, Coworker activity reads `coworkerApi.list` (a roster count, not live activity), and Recent artifacts is UNAVAILABLE because no Workspace-wide artifact projection is served (artifacts list per Task only). NOT done: a live Coworker activity projection, a recent-artifacts projection, a rendered-browser check of the effect-driven states (no Chrome here), and the ui-behavior.md trace row
- **Depends on:** T06, T19
- **Cross-package prerequisites:** Feature 005 Task status, Feature 010 automation status, Feature 009 Coworker activity and Feature 007 recent artifacts as the widget read sources.
- **Owner:** Desktop UI.
- **Allowed paths:** `apps/litecowork-ui/src/`, `specs/011-desktop-shell-ipc-settings-runtime/contracts/ui-behavior.md`, adjacent tests.
- **Contract:** SPEC §12; AC-024; V1-UI-SCREEN-CONTRACT.
- **Change intent:** add a persistent Workspace Dashboard with pinnable widgets for Task status, automation status, Coworker activity and recent artifacts. Each widget reads a served owner projection; an unavailable source renders the widget as unavailable with its reason, never a fabricated value. Pin state persists locally per Workspace.
- **Acceptance:**
  - [ ] The dashboard shows pinnable Task-status, automation-status, Coworker-activity and recent-artifacts widgets from served projections (AC-024).
  - [x] An unavailable widget source renders unavailable with its reason, not empty or fabricated (AC-024).
  - [x] Pin state persists across restart per Workspace.
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** dashboard render and persistence test output in Completion evidence.
- **Stop:** a widget would show a value with no served owner source; render it unavailable.

## 011-T23 - Package VERIFIED for V1

- **Status:** NOT STARTED
- **Depends on:** T01-T22
- **Cross-package prerequisites:** NONE beyond those of T13 and T17.
- **Owner:** Product (repository owner), Runtime, Desktop UI
- **Allowed paths:** this package
- **Contract:** docs/SPECIFICATION-STANDARD.md lifecycle; SPEC §15 all AC IDs
- **Change intent:** Assemble the evidence bundle, move SPEC and WIRING status ACCEPTED/LOCKED -> IMPLEMENTING (when T02 resumes code work) -> VERIFIED, and obtain owner sign-off.
- **Acceptance:**
  - [ ] Every AC from AC-001 to AC-024 has recorded CODE/SYSTEM/USER evidence on both rows.
  - [ ] Owner sign-off recorded with date and revision.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** evidence bundle index in CURRENT-RUN.md; IMPLEMENTATION-STATUS.md row.
- **Stop:** stop if T13 remains BLOCKED; the package cannot be VERIFIED without the stop/update gate.

## Implementation language and optimization notes

- Keep IPC, stop gate, settlement and keep-awake in Rust (`operator-ipc`, `windows-local-security`, `litecoworkd`). The UI renders observations only.
- Keep-awake belongs in the daemon process, not the Tauri shell, because the daemon knows when admitted work runs and survives window close.
- `SettingsView.tsx` receives about 60 props from `App.tsx` (4523 lines). Splitting categories into modules with a typed context (T06) stops every category re-rendering on unrelated App state changes and makes the inert-control audit per-category.
- A single TypeScript shortcut registry (T11) makes "label only when a handler exists" structural.
- `apps/litecowork-ui/src-tauri/src/lib.rs` (6132 lines) registers every command; new stop/update and rename commands go in their own bridge modules, as `runtime_lifecycle_bridge.rs` already does, rather than growing `lib.rs`.

## Dependency order

T02, T03, T04, T05 start immediately (hardening existing code). T01 -> T06 -> T07, T09, T10, T11; T01, T06 -> T08; T01 -> T12; T01, T04 -> T13 (blocked on Features 001 and 005) -> T14; T05, T14 -> T15; T07-T12, T14 -> T16; T02, T03, T15, T16 -> T17 -> T18. Owner-approved additions: T06 -> T19 (shell reachability, P-05), T06 -> T20 (decomposition, P-07+P-19), T06 + T20 -> T21 (Settings merge, P-08), T06 + T19 -> T22 (Workspace Dashboard, P-16). T18, T19, T20, T21, T22 -> T23 (VERIFIED depends on all).

## Cross-package dependencies

| This task | Other package task | Reason |
|---|---|---|
| T02, T03 | Feature 019 native Windows and Linux CI runners | Native-row evidence, never WSL2 |
| T09 | Feature 002 OpenCode binding probe fix and AgentControlDescriptor producer | Agents screen consumes 002 operations (A-35) |
| T10 | Feature 010 quiet-hours control surface | Quiet hours owned by 010 (A-35) |
| T12 | Feature 001 admitted-turn state; Feature 005 admitted-Attempt state | Defines "work runs" for keep-awake |
| T13 | Feature 001 Stop/quiescence (A-25); Feature 005 Effect/lease settlement; Feature 008 session settlement | Stop route requires settlement proof |
| T14 | Feature 019 updater | Update must call the quiescence gate |
| T17 | Feature 019 release matrix and installer; Feature 001 qualified route | Packaged E2E per row |
| (dashboard) | Feature 018 usage and diagnostics; Feature 017 backup | Former Usage task moved; categories render unavailable here |

Cross-package task numbers are named by package and outcome because those packages' TASKS.md are being renumbered in the same release-planning change. Feature 012 sequences after Feature 001 Stop, not after this package.

## DeepSeek Harness proposals (PROPOSED, owner review required)

Status: PROPOSED. None of the items below is an accepted task. They come from the DeepSeek Harness studies ([desktop deep-dive](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) register A-63, proposals P-31 to P-46; [plugin deep-dive](../../docs/DEEPSEEK-HARNESS-PLUGIN-ARCHITECTURE-DEEP-DIVE-2026-11.md) register A-58). Owner review is required before any item becomes a numbered task in this package. They are labelled `PROP-Pnn` so they do not collide with the stable task IDs that `.specify/scripts/validate_feature_package.py` checks. On acceptance: assign the next free task ID in this file, copy the upstream permalink, class and reuse statement into the task, and update the [reuse map](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md). Upstream pin `fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159` (MIT, Copyright (c) 2026 DeepSeek). No upstream code is imported (AGENTS.md section 7).

### Proposed tasks owned by this package

- **PROP-P31 (P-31) QuiescenceReport (one "interruptible work" predicate; unknown means busy; lock then learn; inspect once)** [PROPOSED, not accepted, owner review required]
  - Class: **ADAPT**.
  - Would touch: T12, T13, T14 of package 011; T11 of package 019; T10 of package 010.
  - Upstream permalinks: [src/host-process.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/host-process.ts), [src/quit-confirmation.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/quit-confirmation.ts).
  - Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 3 and 14 (row P-31).
  - Why: LiteCowork describes the same predicate in three places (T12, T13, T14 of package 011). Upstream shows the same facts must back stop, update, quit and keep-awake, and that unknown resolves to busy. The inputs differ (admitted turns, admitted Attempts, Terminal and Browser sessions, armed Automations, pending approvals, not Electron tasks), so it is an ADAPT, not a port.
  - Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React).
  - Deferral note: Daemon half (`RuntimeLifecycleStore`, served `previewLocalRuntimeStop`) can be built and tested on Linux; the packaged quit path needs the Windows laptop. Not started.

- **PROP-P32 (P-32) Per-request deadlines with defined meaning-on-timeout for destructive shell-to-daemon calls** [PROPOSED, not accepted, owner review required]
  - Class: **ADAPT**.
  - Would touch: T04 and T13 of package 011; T07 of package 019.
  - Upstream permalinks: [src/host-process.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/host-process.ts), [src/welcome-window.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/welcome-window.ts), [src/update-dialog.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/update-dialog.ts).
  - Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 2, 6 and 14 (row P-32).
  - Why: LiteCowork has IPC admission bounds (T04 of package 011) but no stated timeout meaning for stop, update and quit calls. The upstream rule (a timeout on a destructive gate means do not proceed) is reusable; the transport differs (UDS and named pipe, not loopback HTTP).
  - Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React).
  - Deferral note: Not started. Needs the operator IPC and Tauri bridge decisions in package 011 first.

- **PROP-P37 (P-37) Golden files for user-visible dialog copy and golden scenario lists for installer tests** [PROPOSED, not accepted, owner review required]
  - Class: **ADAPT**.
  - Would touch: T19 of package 011; T16 and T08 of package 019.
  - Upstream permalinks: [tests/expected/](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/tests/expected), [tests/expected/windows-installer.json](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/tests/expected/windows-installer.json), [tests/expected/windows-uninstall.json](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/tests/expected/windows-uninstall.json), [src/locale.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/locale.ts).
  - Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 12 and 14 (row P-37).
  - Why: A reviewable golden for stop, update, recovery and failure dialog copy is a strong guard for the truthfulness rules (T19 of package 011). The files themselves hold DeepSeek strings and are not reusable; only the technique is.
  - Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React). If a scenario list from `windows-uninstall.json` or `windows-installer.json` is copied verbatim as a test outline, if any text or structure is carried over, MIT applies (upstream LICENSE, "Copyright (c) 2026 DeepSeek"): keep that copyright line and the MIT permission notice in the third-party notices produced by task T13 of package 019, and cite the upstream path and SHA `fcc0ad46` in the code comment.
  - Deferral note: UI golden tests can run on Linux; installer goldens wait for the Windows laptop.

- **PROP-P46 (P-46) IME-safe shortcut dispatch rules (no match during composition, dead keys or AltGr; reset on blur; balanced keyups)** [PROPOSED, not accepted, owner review required]
  - Class: **ADAPT (rules only)**.
  - Would touch: T11 of package 011.
  - Upstream permalinks: [src/keyboard.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/keyboard.ts).
  - Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Section 14 (row P-46).
  - Why: `src/keyboard.ts` imports `electron` and four workspace packages, so it is not reusable as a file; the rules become acceptance cases for the existing `src/shell/shortcut-registry.ts`.
  - Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React).
  - Deferral note: UI tests on Linux; not started.


- **PROP-P47 (A-78) Out-of-app Computer takeover trigger** [PROPOSED, not accepted, owner review required]
  - **Class:** ADAPT; new Desktop shell/native integration surface, no domain authority or new route.
  - **Would touch:** Feature 011 keyboard/native-shell registration and recovery contracts; Feature 008 T22/T25; Tauri bridge and OS-specific tests only after owner review.
  - **Change intent:** While a Feature 008 Computer input lease is AGENT-owned, provide a qualified OS-level global **Take over** shortcut or native tray control that works while LiteCowork is behind the target app. It must invoke Feature 008's same authenticated, nonblocking last-hop input fence; it does not create a second lease. The tray menu may also expose **Stop run** through its separate Conversation/Task owner path. The global shortcut maps to Take over only unless a distinct Stop shortcut receives explicit owner review. Register the trigger before admitting agent input and refuse mediated input if no supported trigger is active. Define shortcut conflicts, registration loss, tray availability, shell/Runtime restart, daemon disconnect, accessibility and per-OS qualification.
  - **Acceptance proposal:** prove the action works with the target app foreground; confirm queued/stale input is fenced; show pending/unknown if acknowledgement or a dispatched OS action is unresolved; deny input when registration or control channel is unavailable; never claim a popup is sufficient. Qualify Windows and each supported Linux desktop session independently.
  - **Stop:** no numbered implementation task until Desktop shell, Environments and Trust owners resolve A-78 and Feature 008 revision 1.1 is accepted. Current shell source has no wired global Computer shortcut/tray action.

## Completion evidence

For each task record in CURRENT-RUN.md and the matching docs/IMPLEMENTATION-STATUS.md row: task ID, git revision, exact command, pass/fail with test names, OS and build (Windows 11 or 10 x64 build number, or Linux x64 distribution and kernel; never WSL2), app build, harness and version where relevant (OpenCode v2.0.32), evidence level reached (CODE, SYSTEM or USER), and reviewer. A task is DONE only when its acceptance boxes are checked against that record.
