# Tasks - Workbench Browser Terminal and Computer

**Feature:** 008 | **Status:** ACCEPTED/LOCKED - implementation authorized; tasks not started unless marked.
**Spec revision:** 1.0 | **Release target:** V1 (release matrix rows: Windows x64 + OpenCode v2.0.32 first; Linux x64 second)

## Historical baseline (2026-11-11; superseded)

Verified in source on 2026-11-11:

- `apps/litecowork-ui/src/workbench/WorkbenchPanel.tsx` (608 lines) is mounted by `App.tsx` as the right pane. After register A-51, `TerminalPane` renders an "unavailable" notice with no input or output, `ReviewPane` renders "Nothing to review yet", and the Browser iframe starts with an empty `browserUrl` labelled "Embedded page preview". `apps/litecowork-ui/tests/workbench-truthfulness.test.mjs` guards against the strings `exit code`, `ConPTY`, `Job Object`, `KILL_ON_CLOSE`, `pwsh` and `DACL`.
- Activity now has a launcher/tab and its own status-only empty state; Computer use is shown as status-only UNAVAILABLE with no action control.
- `apps/litecoworkd/src/browser_consent.rs` (`create_managed_profile`, `is_default_chrome_user_data_directory`, `decide_existing_chrome_attach`, `authorize_observation`) and `apps/litecoworkd/src/browser_control_service.rs` (`admit_action`, `request_takeover`, `return_to_agent`, `BrowserControlLease`, `BrowserControlError`) are pure policy code compiled under `#[allow(dead_code)]` in `apps/litecoworkd/src/main.rs`; only their unit tests call them.
- `apps/litecoworkd/src/environment_computer_use.rs` `probe_computer_provider()` never reports READY; every capture/input request is refused; also `#[allow(dead_code)]`.
- `crates/windows-local-security/src/lib.rs` has `spawn_kill_on_drop_job`, `ContainedChild::kill_tree` and `wait_for_job_quiescence` (process lifetime only). No `openpty`, `CreatePseudoConsole`, TerminalSessionService, terminal route or Tauri terminal bridge exists. `apps/litecoworkd/src/environment_isolation.rs` probes for bubblewrap but is not wired to Task admission.
- `apps/litecowork-ui/src/artifacts/text-version-diff-model.ts` `lineDiff` is a bounded LCS used by Artifact comparison, not by the Workbench.
- `docs/schemas/served-route-inventory.json` has no BrowserSession, TerminalSession, Activity, Diff or Computer route; `takeEnvironmentControl`/`returnEnvironmentControl` are target-only.

Gaps: every provider, store, route, bridge and real view listed in SPEC §7.2; the Activity view; real Diffs; per-row terminal availability; Computer UNAVAILABLE surface.

The preceding baseline predates the 2026-11-11 handoff changes and is retained for provenance. At the 2026-11-12 checkout, Workbench Activity and Computer remain truthful status-only surfaces, Terminal reports platform blockers, Browser is still an empty embedded-page preview, and Review has no real VCS diff. The managed-browser observation viewport and takeover described in the UI contract are TARGET only. No browser, terminal or computer provider journey is implemented or qualified; use the package task statuses and `docs/IMPLEMENTATION-STATUS.md` for current evidence.

## 008-T01 - Promote accepted Workbench contracts into owner docs and schemas

- **Status:** NOT STARTED
- **Depends on:** NONE
- **Cross-package prerequisites:** Feature 005 Environment admission contract (the terminal binds to its admitted Environment identity); Feature 003 ResourceRevision identity (Diff sources).
- **Owner:** Desktop shell, Browser provider, Environments, Trust (contract owners)
- **Allowed paths:** `docs/CHROME-BROWSER-LLD.md`, `docs/V1-INTEGRATED-TERMINAL-LLD.md`, `docs/ENVIRONMENTS.md`, `docs/EVENTS.md`, `docs/schemas/operator-api.openapi.yaml`, `docs/schemas/domain-event.schema.json`, `docs/schemas/error-codes.schema.json`, `docs/schemas/sqlite-v41.sql` (new, allocated in docs/LITECOWORK-BUILD-TASKS.md; v29 is already taken by Feature 001), `apps/litecowork-ui/src/generated/` (regenerated `operator-api.ts` via `pnpm --dir apps/litecowork-ui run api:generate`), this package
- **Contract:** SPEC §5 (B-22, B-23, B-32), §7.2, §15; AC-002, AC-004, AC-005, AC-009, AC-013, AC-014
- **Change intent:** In one reviewed change, promote: pinned Chromium channel/version and managed-profile policy; BrowserSession, BrowserControlLease, ChromeAttachConsent epoch and BrowserActionReceipt operations; TerminalSession create/attach/input/resize/stop, ShellProfile discovery, `CommandReceipt` and `QuiescenceReceipt`; Workbench Activity read and RevisionDiff read; events (`environment.control.lease.changed.v1` promoted from target, plus terminal session/receipt events); error codes mapping `BrowserControlError` reason codes and `TerminalError`; one immutable SQLite migration for the session/lease/receipt tables. Each operationId is marked target-only in the route inventory until its handler lands.
- **Acceptance:**
  - [ ] OpenAPI, event schema, error-code schema and migration validate and match the owner LLD prose field for field.
  - [ ] No payload carries a raw CDP endpoint, PTY handle, OS process handle, profile path or environment value (redaction review).
  - [ ] Generated TypeScript types regenerate cleanly (`api:check` passes).
  - [ ] `scripts/audit_served_routes.py` still reports the new operations as target-only.
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `pnpm --dir apps/litecowork-ui run api:check` and `python3 scripts/audit_served_routes.py`
- **Evidence:** owner-doc diff and schema validation output recorded in CURRENT-RUN.md; register row for B-22/B-23 promotion.
- **Stop:** stop if any promoted field conflicts with an owner doc or a second schema authority would be created.

## 008-T02 - Finish Workbench truthfulness: Activity fallthrough, Computer UNAVAILABLE, per-row reasons

- **Status:** PARTIAL (2026-11-11 CODE: Activity has an explicit status-only empty state and tab branch; Computer use is status-only; TerminalPane names the distinct Windows/Linux Environment blocker. Focused Workbench tests passed 11/11, `pnpm exec tsc --noEmit` passed, and `pnpm run build` passed after confirming generated API output matches the schema. Full UI suite now passes **633/633** (`node --experimental-transform-types --test --test-concurrency=1 --test-reporter=tap "tests/*.test.ts" "tests/*.test.mjs"`) after correcting stale Workspace/Runtime selectors and warming Vite's production harness module before browser startup. Static accessibility checks cover notice names/focusability, Activity order and the existing narrow-pane Escape handler; no axe browser audit was run. Still open: a rendered accessibility audit.)
- **Depends on:** NONE
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop UI
- **Allowed paths:** `apps/litecowork-ui/src/workbench/WorkbenchPanel.tsx`, `apps/litecowork-ui/src/workbench/workbench-panel.css`, `apps/litecowork-ui/tests/workbench-truthfulness.test.mjs`
- **Contract:** SPEC §4, §5 B-32, §15; contracts/ui-behavior.md state table; AC-001, AC-011, AC-016
- **Change intent:** Add an explicit `ActivityPane` that renders an empty-state "No recorded activity source is served by this build" until T14, so `activeTab === "activity"` never renders `ReviewPane`. Add a `ComputerStatusNotice` (status text only, no button) stating Computer use is not available in V1. Give `TerminalPane` a `platform` prop so the Windows reason names the missing Windows Environment provider and the Linux reason names the missing admitted Environment. Keep every `UnavailableNotice` a `role="status"` region reachable by keyboard.
- **Acceptance:**
  - [ ] Rendering with `activeTab="activity"` shows the Activity empty state, not Review (AC-011).
  - [ ] Computer appears only as status text; no element with a Computer action role exists (AC-016).
  - [ ] Existing `FABRICATED_PROCESS_CLAIMS` assertions still pass; the iframe is never labelled Chrome (AC-001).
  - [ ] Axe-style checks: each notice has an accessible name, focus order follows the launcher, Escape closes the narrow-window Workbench (AC-011).
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test names in `workbench-truthfulness.test.mjs` (filter to be added by this task: `activity tab renders its own empty state`, `computer is status-only`); IMPLEMENTATION-STATUS.md Workbench row.
- **Stop:** stop if a change would add any control whose owner operation is not served.

## 008-T03 - Browser session, lease and receipt storage with CAS and idempotency

- **Status:** NOT STARTED
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Storage, Browser provider
- **Allowed paths:** `crates/storage-core/src/browser_session.rs` (new), `crates/storage-core/src/lib.rs`, `crates/storage-sqlite/src/browser_sessions.rs` (new), `crates/storage-sqlite/src/lib.rs`, `crates/storage-sqlite/src/migrations/` (the T01 migration), `crates/storage-sqlite/tests/`
- **Contract:** SPEC §7, §11; DATA-MODEL; CHROME-BROWSER-LLD target data; AC-002, AC-004
- **Change intent:** Add a typed port `BrowserSessionStore` with `create_session`, `record_consent`, `transfer_control` (compare-and-swap on `expected_epoch` and `expected_session_version`), `record_action_receipt` (states REJECTED/ADMITTED/IN_PROGRESS/APPLIED/AMBIGUOUS/BLOCKED) and `mark_incarnation_lost`. Every write is one SQLite IMMEDIATE transaction with event and idempotency receipt; no network or process call inside.
- **Acceptance:**
  - [ ] Consent row commits before any ADMITTED receipt for the same epoch (AC-002).
  - [ ] Stale epoch, stale session version, foreign Workspace and wrong principal fail with no partial write (AC-004).
  - [ ] Same idempotency key and payload returns the original receipt; same key with a different payload fails.
  - [ ] Two concurrent `transfer_control` calls on one epoch: exactly one wins.
  - [ ] Crash between statements (fault injection) leaves no half-applied epoch.
- **Command:** `cargo test -p storage-sqlite --locked browser_session_store` (filter to be added by this task)
- **Evidence:** test output in CURRENT-RUN.md.
- **Stop:** stop if the schema needs a field not promoted in T01.

## 008-T04 - Managed Chromium provider with isolated profile (B-22)

- **Status:** NOT STARTED
- **Reference (DeepSeek Harness, MIT, pinned `fcc0ad46`, no code copied yet):** upstream [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159), reuse map [DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md).
  - **P-45 Default-deny webview and session permissions, including the main one, plus a guest network filter.** Class: **ADAPT**. Upstream permalinks: [src/browser-guests.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/browser-guests.ts), [src/microphone-permissions.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/microphone-permissions.ts), [src/policy-test-auth.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/policy-test-auth.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 9 and 14 (row P-45). Why: Upstream shows a counter-example (the shell main session grants non-media permissions). Tauri has no equivalent default grant; keep it that way and add a test that notification, clipboard-read and geolocation requests are denied. The CSP audit already exists in `scripts/v1_csp_audit.py`. Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React).
- **Depends on:** T01
- **Cross-package prerequisites:** NONE (browser does not need a Task Environment; Runtime process supervision from Feature 011 lifecycle applies)
- **Owner:** Browser provider, Security
- **Allowed paths:** `apps/litecoworkd/src/browser_provider.rs` (new), `apps/litecoworkd/src/browser_consent.rs`, `apps/litecoworkd/src/main.rs`, `apps/litecoworkd/Cargo.toml`, `crates/windows-local-security/src/lib.rs` (process-lifetime Job Object reuse only)
- **Contract:** SPEC §5 B-22, §10; CHROME-BROWSER-LLD; docs/REFERENCE-IMPLEMENTATIONS.md REF-BROWSER-HARNESS adapter rules; AC-008
- **Change intent:** Launch the pinned Chromium only with `--user-data-dir` from `browser_consent::create_managed_profile` and refuse any path where `is_default_chrome_user_data_directory` is true. Bind CDP to a private pipe or loopback port known only to the provider; never scan for ambient CDP endpoints (no equivalent of browser-harness `get_ws_url()`), never import cookies/history/login, never forward generic CDP (no equivalent of `handle()`); expose only an allowlisted typed method set (navigate, screenshot, DOM/accessibility snapshot, click, type, scroll). On Windows, wrap the process with `spawn_kill_on_drop_job` for lifetime only. Remove `#[allow(dead_code)]` from `browser_consent` once used.
- **Acceptance:**
  - [ ] Launch with a default-profile path is refused with a typed error (AC-008).
  - [ ] No code path enumerates `default_chrome_user_data_directories()` for attach; test asserts no CDP discovery call.
  - [ ] Password/payment/CAPTCHA/MFA regions classified by `classify_sensitive_region` reject agent input (AC-008).
  - [ ] Provider crash is observed and reported as provider loss within a bounded deadline.
  - [ ] Pinned Chromium version mismatch reports NEEDS_SETUP, not READY.
- **Command:** `cargo test -p litecoworkd --locked browser_provider` (filter to be added by this task)
- **Evidence:** test output plus a profile-directory listing showing only the managed marker, recorded in CURRENT-RUN.md.
- **Stop:** stop if the pinned Chromium distribution or its license is not recorded in RESEARCH.md.

## 008-T05 - BrowserSessionService, Operator routes and Tauri bridge

- **Status:** NOT STARTED
- **Depends on:** T03, T04
- **Cross-package prerequisites:** Feature 011 authenticated IPC gate (principal from transport only).
- **Owner:** Browser provider, Trust, Operator, Desktop shell
- **Allowed paths:** `apps/litecoworkd/src/browser_session_operator.rs` (new), `apps/litecoworkd/src/browser_control_service.rs`, `apps/litecoworkd/src/operator.rs` (route mount only), `apps/litecoworkd/src/main.rs`, `apps/litecowork-ui/src-tauri/src/browser_session_bridge.rs` (new), `apps/litecowork-ui/src-tauri/src/lib.rs` (handler registration), `docs/schemas/served-route-inventory.json` (regenerated)
- **Contract:** SPEC §6 actor sequence, §7; CHROME-BROWSER-LLD; AC-002, AC-003, AC-004
- **Change intent:** Compose `browser_control_service::admit_action`, `request_takeover` and `return_to_agent` with the T03 store and T04 provider. Order: authenticate, authorize Workspace scope, CAS the epoch, commit receipt, then dispatch to the provider, then reconcile. Mount the T01 operationIds. Add Tauri commands that forward to the Operator over the authenticated IPC client; the WebView never supplies a principal. Remove `#[allow(dead_code)]` from `browser_control_service`.
- **Acceptance:**
  - [ ] Agent action without a recorded consent epoch is rejected before provider dispatch (AC-002).
  - [ ] Takeover advances the epoch atomically and discards queued agent input; a queued input submitted after takeover is rejected (AC-003).
  - [ ] An agent handle from epoch n is rejected after the epoch moves to n+1 (AC-004).
  - [ ] Return requires explicit confirmation and a fresh observation before the agent may act.
  - [ ] Unauthenticated or wrong-Workspace caller is denied with the promoted error code.
- **Command:** `cargo test -p litecoworkd --locked browser_epoch` (filter to be added by this task) and `cargo test -p litecowork-ui --locked browser_session_bridge` (filter to be added by this task)
- **Evidence:** test output; served-route inventory diff.
- **Stop:** stop if any route accepts a client-supplied actor as authority.

## 008-T06 - Browser Workbench view with consent, take control and return

- **Status:** NOT STARTED
- **Reference (DeepSeek Harness, MIT, pinned `fcc0ad46`, no code copied yet):** upstream [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159), reuse map [DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md).
  - **P-45 Default-deny webview and session permissions, including the main one, plus a guest network filter.** Class: **ADAPT**. Upstream permalinks: [src/browser-guests.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/browser-guests.ts), [src/microphone-permissions.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/microphone-permissions.ts), [src/policy-test-auth.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/policy-test-auth.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 9 and 14 (row P-45). Why: Upstream shows a counter-example (the shell main session grants non-media permissions). Tauri has no equivalent default grant; keep it that way and add a test that notification, clipboard-read and geolocation requests are denied. The CSP audit already exists in `scripts/v1_csp_audit.py`. Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React).
- **Depends on:** T02, T05
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop UI
- **Allowed paths:** `apps/litecowork-ui/src/workbench/BrowserPane.tsx` (new), `apps/litecowork-ui/src/workbench/WorkbenchPanel.tsx`, `apps/litecowork-ui/src/workbench/browser-session-api.ts` (new), `apps/litecowork-ui/src/workbench/workbench-panel.css`, `apps/litecowork-ui/tests/workbench-browser-session.test.mjs` (new)
- **Contract:** contracts/ui-behavior.md state table; CHROME-BROWSER-LLD UI; AC-001, AC-003, AC-011
- **Change intent:** Render the provider-backed browser view (LIVE_INTERACTIVE or OBSERVATION_FRAMES) separately from the existing "Embedded page preview" iframe, which keeps its label and gains no agent badge. For the accepted observation mode, show only the managed browser tab viewport as ephemeral frames with bounded latest-frame backpressure and stale-frame labeling; never describe it as laptop/desktop sharing. Show mode, engine/version, control owner and epoch. Consent dialog before the first agent action. While a live lease is AGENT-owned, show a persistent, non-modal **Take over** banner. Its UI reports USER control only after the provider confirms the new epoch and old-epoch inputs are fenced; reconcile dispatched actions before showing a settled result. This browser takeover does not stop Conversation generation or a Task. Feature 012 Conversation Take control remains a separate cancel/quiescence action. Return is a separate confirmation and starts from a fresh observation. States: starting, NEEDS_SETUP, UNAVAILABLE, OFFLINE, provider error with repair, consent denied, stale epoch.
- **Acceptance:**
  - [ ] With no provider, the view shows UNAVAILABLE and never the word Chrome (AC-001).
  - [ ] Take control disables agent input only after the Operator confirms the new epoch (AC-003).
  - [ ] Browser takeover is visible only for an AGENT-owned live browser lease and does not imply that Conversation generation or a Task stopped (Feature 012 Take control is distinct).
  - [ ] No whole-desktop/laptop capture control is shown under accepted B-32; without a served browser provider, show status only.
  - [ ] All controls have accessible names; status changes are announced through a live region (AC-011).
  - [ ] Closing the pane does not close the browser session or stop a Task.
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test names in `workbench-browser-session.test.mjs`; UI ledger row.
- **Stop:** stop if the view would render controls before the T05 route is served.

## 008-T07 - Browser failure and recovery: provider loss, restart, races, replay

- **Status:** NOT STARTED
- **Depends on:** T05
- **Cross-package prerequisites:** Feature 011 Runtime incarnation and drain contract.
- **Owner:** Browser provider, Runtime
- **Allowed paths:** `apps/litecoworkd/src/browser_session_operator.rs`, `apps/litecoworkd/src/browser_provider.rs`, `apps/litecoworkd/src/runtime.rs` (startup reconcile hook only), `apps/litecoworkd/tests/` (new browser recovery tests)
- **Contract:** SPEC §11, §12; contracts/errors-recovery.md; AC-003, AC-007, AC-017
- **Change intent:** On provider loss mark the epoch LOST/RECOVERING and every in-flight receipt AMBIGUOUS; add `reconcile_browser_action` that compares a fresh observation and never resubmits a form. At startup, call `mark_incarnation_lost` for sessions of the previous incarnation and reject their handles. Takeover racing an in-flight input waits for drain or cancels it before the epoch increments.
- **Acceptance:**
  - [ ] Killing Chromium mid-action yields AMBIGUOUS, not APPLIED; retry is refused until reconciled (AC-007).
  - [ ] After daemon restart, a handle from the old incarnation is rejected (AC-017).
  - [ ] Takeover during an in-flight click: the click is either drained with a receipt or cancelled; never applied after the epoch change (AC-003).
  - [ ] Duplicate action replay with the same idempotency key returns the original receipt.
  - [ ] A matching URL after restart is not treated as success of a pre-crash write.
- **Command:** `cargo test -p litecoworkd --locked browser_recovery` (filter to be added by this task)
- **Evidence:** fault-injection test output in CURRENT-RUN.md.
- **Stop:** stop if any recovery path retries a consequential action before reconciliation.

## 008-T08 - Terminal session and ordered receipt storage

- **Status:** NOT STARTED
- **Depends on:** T01
- **Cross-package prerequisites:** NONE
- **Owner:** Storage, Environments
- **Allowed paths:** `crates/storage-core/src/terminal_session.rs` (new), `crates/storage-core/src/lib.rs`, `crates/storage-sqlite/src/terminal_sessions.rs` (new), `crates/storage-sqlite/src/lib.rs`, `crates/storage-sqlite/tests/`
- **Contract:** SPEC §7.2 `CommandReceipt`, `QuiescenceReceipt`; V1-INTEGRATED-TERMINAL-LLD; AC-009, AC-010
- **Change intent:** Add `TerminalSessionStore` with `open_session` (bound to `environment_id`, shell profile identity, `owner_kind` USER_MANUAL/TASK_ATTEMPT), `append_command_receipt` (monotonic `order`, persisted before publication), `record_quiescence` (QUIESCENT/LOST/UNKNOWN) and `mark_incarnation_lost`. Raw terminal bytes are not stored in replicated events.
- **Acceptance:**
  - [ ] `order` is strictly increasing per session; a duplicate order is rejected (AC-009).
  - [ ] A timeout receipt stores `exit_code = -1` and `timed_out = true`; no receipt can store exit 0 without an observed exit (AC-009).
  - [ ] Session for a foreign or non-admitted Environment is rejected.
  - [ ] Concurrent appends from two writers keep a gap-free order.
  - [ ] STOPPING without a quiescence observation stays STOPPING/UNKNOWN (AC-010).
- **Command:** `cargo test -p storage-sqlite --locked terminal_session_store` (filter to be added by this task)
- **Evidence:** test output in CURRENT-RUN.md.
- **Stop:** stop if receipts could be published before commit.

## 008-T09 - Safe shell discovery and Linux PTY under an admitted bubblewrap Environment

- **Status:** BLOCKED (needs an admitted Linux bubblewrap Environment from Feature 005, owner Environments)
- **DEFERRED/BLOCKED note:** Why: needs an admitted Linux bubblewrap Environment from Feature 005. Unblocks when: Feature 005 Linux Environment admission. Test later at: native Linux desktop with bubblewrap installed.
- **Depends on:** T08
- **Cross-package prerequisites:** Feature 005 Linux bubblewrap Environment admission and attestation task; Feature 005 B-17 qualification.
- **Owner:** Environments, Security
- **Allowed paths:** `apps/litecoworkd/src/terminal_pty_linux.rs` (new), `apps/litecoworkd/src/shell_profiles.rs` (new), `apps/litecoworkd/Cargo.toml` (add the `rustix` `pty` feature), `crates/linux-process-scope/src/`
- **Contract:** SPEC §5 B-23 rider; V1-INTEGRATED-TERMINAL-LLD discovery and PTY protocol; ENVIRONMENTS; AC-005, AC-006
- **Change intent:** `detect_shell_profiles()` resolves executables and runs only a bounded `--version` probe with an empty environment; never sources rc files, never reads env secrets. `spawn_in_environment(env, profile)` opens a PTY via `rustix::pty` and spawns exactly one process tree inside the admitted bubblewrap Environment with exact argv, cwd inside an authorized root (no silent fallback to home) and an allowlisted environment, placing it in a `linux-process-scope` cgroup for lifetime. Returns a containment receipt naming the Environment attestation.
- **Acceptance:**
  - [ ] Discovery with a booby-trapped `.bashrc` performs no write and spawns no shell (AC-006).
  - [ ] Each tab gets one process tree in its own Environment; two Environments never share a tree (AC-005).
  - [ ] Unauthorized cwd is rejected; no fallback to `$HOME`.
  - [ ] Spawn without an admitted Environment returns `EnvironmentNotAdmitted`.
- **Command:** `cargo test -p litecoworkd --locked terminal_environment_bind` (filter to be added by this task) and `cargo test -p linux-process-scope --locked`
- **Evidence:** Linux x64 test output with Environment attestation id, CURRENT-RUN.md.
- **Stop:** stop if the Environment is not attested ENFORCED for the current Runtime incarnation.

## 008-T10 - TerminalSessionService, Operator routes, stream bridge and backpressure

- **Status:** NOT STARTED
- **Depends on:** T08, T09
- **Cross-package prerequisites:** Feature 011 authenticated IPC gate.
- **Owner:** Environments, Operator, Desktop shell
- **Allowed paths:** `apps/litecoworkd/src/terminal_session_operator.rs` (new), `apps/litecoworkd/src/operator.rs` (route mount only), `apps/litecowork-ui/src-tauri/src/terminal_session_bridge.rs` (new), `apps/litecowork-ui/src-tauri/src/lib.rs` (handler registration), `docs/schemas/served-route-inventory.json` (regenerated)
- **Contract:** SPEC §7.2 `TerminalSessionService`; V1-INTEGRATED-TERMINAL-LLD stream protocol; AC-009, AC-012
- **Change intent:** Implement `open`, `write_stdin`, `resize`, `stop` and `attach(cursor)`. Output goes into a bounded ring buffer per session; the stream bridge delivers frames with a cursor so reattach resumes without respawn. When a consumer lags, drop to a resync marker rather than block the PTY reader or the Operator. Command receipts use the OpenHands doctrine: ordered, persisted before publication, timeout as `exit_code = -1`.
- **Acceptance:**
  - [ ] A command that exceeds its deadline yields a timeout receipt, never a fabricated success (AC-009).
  - [ ] 50 MiB of output on one tab leaves another tab and Operator readiness responsive within the IPC handler bound (AC-012).
  - [ ] Reattach from a cursor returns the missing frames or an explicit gap marker.
  - [ ] Unauthenticated caller and wrong Workspace are denied.
- **Command:** `cargo test -p litecoworkd --locked terminal_session_service` (filter to be added by this task) and `cargo test -p litecowork-ui --locked terminal_session_bridge` (filter to be added by this task)
- **Evidence:** throughput test numbers and receipt test output in CURRENT-RUN.md.
- **Stop:** stop if the PTY reader can block on a slow UI consumer.

## 008-T11 - Terminal Workbench view with per-row availability

- **Status:** NOT STARTED
- **Depends on:** T02, T10
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop UI
- **Allowed paths:** `apps/litecowork-ui/src/workbench/TerminalSessionPane.tsx` (new), `apps/litecowork-ui/src/workbench/WorkbenchPanel.tsx`, `apps/litecowork-ui/src/workbench/terminal-session-api.ts` (new), `apps/litecowork-ui/package.json` (terminal emulator dependency, pinned), `apps/litecowork-ui/tests/workbench-terminal-session.test.mjs` (new)
- **Contract:** contracts/ui-behavior.md state table; V1-INTEGRATED-TERMINAL-LLD UI; AC-011, AC-015
- **Change intent:** On the Linux row, render the real session: shell profile/version, Environment id, process state (STARTING/ACTIVE/EXITED/RECONNECTING/LOST/DETACHED/STOPPING), separate Close view and Stop process controls, exit status only from receipts. On the Windows row keep `TerminalPane` UNAVAILABLE with the Feature 005 B-35 reason until T12. The pane is lazy-loaded so the unavailable path loads no terminal emulator code.
- **Acceptance:**
  - [ ] Windows build renders UNAVAILABLE naming the Windows Environment provider and issues no terminal command (AC-015).
  - [ ] Linux build without an admitted Environment renders UNAVAILABLE with that reason (AC-015).
  - [ ] Shell without integration shows exit status NONE, never a guess.
  - [ ] Keyboard focus can enter and leave the terminal (documented escape key) and all controls have names (AC-011).
- **Command:** `pnpm --dir apps/litecowork-ui test` and `pnpm --dir apps/litecowork-ui run build`
- **Evidence:** test names in `workbench-terminal-session.test.mjs`; UI ledger row.
- **Stop:** stop if a Windows build could reach a spawn path.

## 008-T12 - Windows ConPTY provider under a qualified Windows Environment

- **Status:** BLOCKED (Feature 005 blocker B-35, Windows Environment provider, PROPOSED, owner Environments/Security)
- **DEFERRED/BLOCKED note:** Why: needs a qualified Windows Environment (B-35, Feature 005 task T17). Unblocks when: 005-T17 done. Test later at: the owner's Windows laptop (ConPTY).
- **Depends on:** T10
- **Cross-package prerequisites:** Feature 005 B-35 Windows Environment provider task (AppContainer + restricted token + Job Object + WFP egress, all ENFORCED and attested).
- **Owner:** Environments, Security
- **Allowed paths:** `apps/litecoworkd/src/terminal_pty_windows.rs` (new), `crates/windows-local-security/src/lib.rs`, `crates/windows-local-security/Cargo.toml`
- **Contract:** SPEC §5 B-23 rider; V1-INTEGRATED-TERMINAL-LLD; AC-005, AC-015
- **Change intent:** Add `CreatePseudoConsole`-based spawn in `windows-local-security` that launches the shell inside the B-35 AppContainer/restricted token and the existing `spawn_kill_on_drop_job` Job Object, returning `kill_tree` and `wait_for_job_quiescence` evidence. The Windows `TerminalPane` becomes available only when the B-35 attestation is present for the current incarnation.
- **Acceptance:**
  - [ ] Without a B-35 attestation the Windows terminal stays UNAVAILABLE (AC-015).
  - [ ] With attestation, one ConPTY process tree per tab inside its Environment (AC-005).
  - [ ] No document or UI copy calls the Job Object a sandbox.
- **Command:** `cargo test -p windows-local-security --locked conpty` (filter to be added by this task) on native Windows x64
- **Evidence:** native Windows test output (not WSL2) in CURRENT-RUN.md.
- **Stop:** stop if any dimension of the B-35 attestation is not ENFORCED.

## 008-T13 - Terminal failure and recovery: stop, close, crash, restart, races

- **Status:** NOT STARTED
- **Depends on:** T10
- **Cross-package prerequisites:** Feature 011 Runtime drain and stop/quiescence gate.
- **Owner:** Environments, Runtime
- **Allowed paths:** `apps/litecoworkd/src/terminal_session_operator.rs`, `apps/litecoworkd/src/terminal_pty_linux.rs`, `apps/litecoworkd/src/runtime.rs` (startup reconcile hook only), `apps/litecoworkd/tests/` (new terminal recovery tests)
- **Contract:** SPEC §11, §12; contracts/errors-recovery.md; AC-010, AC-017
- **Change intent:** Stop sends graceful termination then escalates, waits for process-tree quiescence and commits QUIESCENT; if quiescence cannot be observed, commits UNKNOWN. UI close is detach only. At startup, prior-incarnation sessions become LOST and their cursors are rejected. A Stop that races spawn is recorded and applied when the process exists.
- **Acceptance:**
  - [ ] Explicit Stop yields QUIESCENT with the tree empty; a stuck child yields UNKNOWN, never "exited" (AC-010).
  - [ ] Closing the view leaves the process running and reattach works (AC-010).
  - [ ] Daemon kill -9 then restart: session shows LOST and old cursor is rejected (AC-017).
  - [ ] Stop racing spawn never leaves an orphan process.
  - [ ] Runtime drain stops terminal admission before DRAINING.
- **Command:** `cargo test -p litecoworkd --locked terminal_recovery` (filter to be added by this task)
- **Evidence:** close/stop/crash evidence matrix in CURRENT-RUN.md.
- **Stop:** stop if any path reports continuation without a surviving process.

## 008-T14 - Workbench Activity view from committed records

- **Status:** NOT STARTED
- **Depends on:** T01, T02
- **Cross-package prerequisites:** Feature 001 committed Conversation events; Feature 005 Task/Attempt events (Task-bound Activity only).
- **Owner:** Desktop UI, Operator
- **Allowed paths:** `apps/litecoworkd/src/workbench_activity_operator.rs` (new), `apps/litecoworkd/src/operator.rs` (route mount only), `apps/litecowork-ui/src-tauri/src/workbench_activity_bridge.rs` (new), `apps/litecowork-ui/src-tauri/src/lib.rs`, `apps/litecowork-ui/src/workbench/ActivityPane.tsx` (new), `apps/litecowork-ui/src/workbench/WorkbenchPanel.tsx`, `apps/litecowork-ui/tests/workbench-activity.test.mjs` (new)
- **Contract:** SPEC §15 AC-013; V1-UI-SCREEN-CONTRACT Activity row; EVENTS
- **Change intent:** A read-only paged projection `WorkbenchActivityItem` over committed domain events and receipts for the bound Conversation or Task (Workspace-scoped, cursor-paged), including browser and terminal receipts once T05/T10 exist. Add the Activity pill and launcher row. Provider reports carry assurance REPORTED; nothing is synthesized.
- **Acceptance:**
  - [ ] With no events, the view shows "No recorded activity yet" (AC-013).
  - [ ] Each row links to a committed `source_event_id`; a test fails if an item lacks one (AC-013).
  - [ ] Cross-Workspace events never appear (denial test).
  - [ ] Stale cursor after restart reloads from the start, no duplicates.
- **Command:** `cargo test -p litecoworkd --locked workbench_activity` (filter to be added by this task) and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** test output; UI ledger row.
- **Stop:** stop if the projection would need a new write path or table.

## 008-T15 - Exact-revision and VCS Diffs

- **Status:** NOT STARTED (an unrelated bounded LCS `lineDiff` exists in `apps/litecowork-ui/src/artifacts/text-version-diff-model.ts` for Artifact comparison)
- **Depends on:** T01, T02
- **Cross-package prerequisites:** Feature 003 ResourceRevision content read; Feature 005 Attempt output change set (VCS diffs of Task edits); Feature 007 Artifact revision identity.
- **Owner:** Desktop UI, Resource store
- **Allowed paths:** `apps/litecoworkd/src/revision_diff_operator.rs` (new), `apps/litecoworkd/src/operator.rs` (route mount only), `apps/litecoworkd/Cargo.toml` (diff crate, license recorded), `apps/litecowork-ui/src-tauri/src/revision_diff_bridge.rs` (new), `apps/litecowork-ui/src-tauri/src/lib.rs`, `apps/litecowork-ui/src/workbench/WorkbenchPanel.tsx` (`ReviewPane`), `apps/litecowork-ui/tests/workbench-revision-diff.test.mjs` (new)
- **Contract:** SPEC §15 AC-014; §7.2 `RevisionDiffRequest`
- **Change intent:** Daemon computes a bounded line diff between two exact sources (`ResourceRevision` with sha256 verified, or a VCS commit pair from a Task Attempt worktree) using a Myers/histogram algorithm, returning rows, true counts and both identities. Over-bound, binary, missing or uncommitted sources return typed states. `ReviewPane` renders the result and keeps its empty state when no change set is selected.
- **Acceptance:**
  - [ ] Equal revisions show "No changes" with zero counts (AC-014).
  - [ ] Counts match an independent reference diff on a fixture set (AC-014).
  - [ ] Digest mismatch, binary input and over-bound input return typed states, never partial counts (AC-014).
  - [ ] Foreign-Workspace revision is denied.
- **Command:** `cargo test -p litecoworkd --locked revision_diff` (filter to be added by this task) and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** fixture comparison output; UI ledger row.
- **Stop:** stop if a diff could be produced from document text without exact revision identity.

## 008-T16 - Cross-layer traces for every Workbench user operation

- **Status:** NOT STARTED
- **Depends on:** T06, T11, T13, T14, T15
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop UI, Operator, Environments, Browser provider
- **Allowed paths:** this package (`contracts/ui-behavior.md`, `WIRING.yaml`), `docs/SEMANTIC-SERVED-UI-MAP.md`
- **Contract:** AGENTS.md §4; SPEC §6
- **Change intent:** Record the AGENTS.md §4 trace (control -> UI command -> Tauri command -> operationId -> authorization -> service -> transaction/fence -> event/receipt -> projection -> restart -> evidence) for: start browser session, consent, agent action, take control, return, open terminal, input, resize, stop, close view, view Activity, view Diff, and the Computer status.
- **Acceptance:**
  - [ ] Every listed operation has a complete trace with real symbol names.
  - [ ] No trace has a UI control without a served route or a route without a UI path.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** trace table in contracts/ui-behavior.md.
- **Stop:** stop if any trace link names a symbol that does not exist in source.

## 008-T17 - Qualify E2E-08 on each release-matrix row

- **Status:** NOT STARTED
- **Depends on:** T16
- **Cross-package prerequisites:** Feature 019 release qualification matrix and packaged build; Feature 001 qualified Conversation route on the Windows row.
- **Owner:** QA, Desktop shell, Product (owner USER evidence)
- **Allowed paths:** `apps/litecowork-ui/tests/browser/` (packaged E2E scripts), `scripts/v1_platform_matrix.py` (row entries only)
- **Contract:** docs/ACCEPTANCE-AND-E2E-PLAN.md E2E-08; SPEC §13 rows; AC-001 through AC-017
- **Change intent:** Run the packaged desktop E2E-08 journey (browser/terminal Workbench, takeover/return, cancel) on Windows x64 (browser qualified; terminal and Computer verified UNAVAILABLE) and Linux x64 (browser and terminal qualified; Computer UNAVAILABLE). Include denial (consent refused, stale epoch), provider crash, daemon restart, duplicate replay and Stop races.
- **Acceptance:**
  - [ ] CODE, SYSTEM and owner USER evidence recorded per row with exact build, OS, Chromium version and shell profile.
  - [ ] Stale handle cannot act after takeover and after restart on both rows (AC-004, AC-017).
  - [ ] Windows row shows Terminal UNAVAILABLE and Computer UNAVAILABLE (AC-015, AC-016).
  - [ ] No WSL2 run is counted.
- **Command:** `pnpm --dir apps/litecowork-ui run test:browser` and `python3 scripts/v1_platform_matrix.py`
- **Evidence:** evidence bundle path per row in CURRENT-RUN.md and docs/IMPLEMENTATION-STATUS.md.
- **Stop:** stop if any row lacks owner USER evidence or any denial/recovery case fails.

## 008-T18 - Documentation sync

- **Status:** NOT STARTED
- **Depends on:** T17
- **Cross-package prerequisites:** NONE
- **Owner:** Documentation owner (orchestrator)
- **Allowed paths:** `docs/IMPLEMENTATION-STATUS.md`, `docs/schemas/served-route-inventory.json`, `docs/SEMANTIC-SERVED-UI-MAP.md`, `docs/V1-UI-SCREEN-CONTRACT.md` (Workbench current-state paragraph), `docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md`, this package
- **Contract:** AGENTS.md §1 and §4
- **Change intent:** Regenerate the served-route inventory, update IMPLEMENTATION-STATUS Workbench rows with exact evidence levels per row, replace the stale Workbench paragraph in V1-UI-SCREEN-CONTRACT, close register rows A-08, A-09, A-10 and A-30 obligations that are met, and refresh SPEC §4 and RESEARCH findings.
- **Acceptance:**
  - [ ] Route inventory matches mounted routes.
  - [ ] No document still describes fabricated terminal output as current.
- **Command:** `python3 scripts/audit_served_routes.py` and `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** documentation diff.
- **Stop:** stop if a status claim exceeds recorded evidence.

## 008-T19 - Remove allow-same-origin from the Browser preview iframe and add a guard test (P-06)

- **Status:** NOT STARTED - added by owner approval on 2026-11-09 (docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md). Folds proposed task P-06 from docs/UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md section 9 (finding U-02) into this LOCKED package.
- **Depends on:** T06
- **Cross-package prerequisites:** NONE
- **Owner:** Desktop shell, Security.
- **Allowed paths:** `apps/litecowork-ui/src/workbench/WorkbenchPanel.tsx`, `apps/litecowork-ui/tests/workbench-truthfulness.test.mjs` (or new guard test), adjacent tests.
- **Contract:** SPEC §12, §8; AC-018; CHROME-BROWSER-LLD; register A-51.
- **Change intent:** remove `allow-same-origin` from the Browser preview iframe `sandbox` attribute (or remove the embedded preview entirely), so `allow-scripts` can never combine with `allow-same-origin` and defeat the iframe sandbox, and add a guard test that fails if any iframe in the source carries both `allow-scripts` and `allow-same-origin`. No remote frame renders in a packaged build (the CSP blocks it); this closes the exposure before any future CSP change.
- **Acceptance:**
  - [ ] No iframe in the source combines `allow-scripts` with `allow-same-origin` (AC-018).
  - [ ] A guard test in the workbench-truthfulness suite fails if that attribute pair reappears (AC-018).
  - [ ] The Browser preview still renders an allowed URL or shows its honest unavailable notice; no model markup executes.
- **Command:** `pnpm --dir apps/litecowork-ui test`
- **Evidence:** guard test output in Completion evidence; register A-51 doc-sync note.
- **Stop:** removing the attribute breaks the only working preview; remove the preview instead and record the decision.

## 008-T20 - Git worktree view, VCS diff and user-invoked GitHub PR ops (P-13)

- **Status:** NOT STARTED - added by owner approval on 2026-11-09. Folds proposed task P-13 from docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md section 8 into this LOCKED package.
- **Depends on:** T15
- **Cross-package prerequisites:** Feature 006 Golden Connection capability so GitHub PR operations run behind a brokered Connection (user-invoked only).
- **Owner:** Desktop shell, Environments, Connections.
- **Allowed paths:** `apps/litecoworkd/src/`, `apps/litecowork-ui/src/workbench/`, `specs/008-workbench-browser-terminal-computer/contracts/ui-behavior.md`, adjacent tests.
- **Contract:** SPEC §12; AC-014, AC-019; register A-51; Feature 006 Connection broker.
- **Change intent:** add a Git worktree view and a changed-file and aggregate diff computed from the real VCS (reusing the exact-revision diff path from task T15), plus user-invoked GitHub pull-request operations that run only behind a Feature 006 brokered Connection. PR operations are always an explicit user action; no agent self-approves or auto-opens a PR, and tokens or arbitrary URLs never reach an agent.
- **Acceptance:**
  - [ ] The worktree view lists changed files and shows a changed-file and aggregate diff from the real VCS with true added/removed counts (AC-019).
  - [ ] GitHub PR operations run only through a Feature 006 brokered Connection and only on explicit user action (AC-019).
  - [ ] Negative: a missing, binary or over-bound source shows a typed unavailable state, not an inferred diff (AC-014).
- **Command:** `python3 .specify/scripts/validate_feature_package.py` and `pnpm --dir apps/litecowork-ui test`
- **Evidence:** diff and PR-op test output; contracts/ui-behavior.md rows in Completion evidence.
- **Stop:** an agent could open or approve a PR without the user; keep PR ops user-invoked only.

## 008-T21 - Package VERIFIED for V1

- **Status:** NOT STARTED
- **Depends on:** T01-T20
- **Cross-package prerequisites:** Feature 005 B-35 resolution for the Windows terminal, or an owner-recorded V1 release decision that ships Windows with Terminal UNAVAILABLE.
- **Owner:** Product (repository owner), Desktop shell, Environments
- **Allowed paths:** this package
- **Contract:** docs/SPECIFICATION-STANDARD.md lifecycle; SPEC §15 all AC IDs
- **Change intent:** Assemble the evidence bundle, move SPEC and WIRING status ACCEPTED/LOCKED -> IMPLEMENTING (when T03 starts) -> VERIFIED, and obtain owner sign-off.
- **Acceptance:**
  - [ ] Every AC from AC-001 to AC-019 has recorded CODE/SYSTEM/USER evidence on its declared row.
  - [ ] Owner sign-off recorded with date and revision.
- **Command:** `python3 .specify/scripts/validate_feature_package.py`
- **Evidence:** evidence bundle index in CURRENT-RUN.md; IMPLEMENTATION-STATUS.md row.
- **Stop:** stop if T12 is still BLOCKED and the owner has not recorded the Windows-terminal-UNAVAILABLE release decision.

## Implementation language and optimization notes

- Keep all process, PTY, browser-CDP, diff and receipt logic in Rust (`litecoworkd`, `windows-local-security`, `linux-process-scope`). TypeScript renders only.
- Diffs: move Workbench diff computation to the daemon (T15). The UI `lineDiff` builds a full O(n x m) LCS table on the WebView thread; a Rust Myers/histogram diff is O((n+m)D) and keeps untrusted bytes out of the renderer. Keep `lineDiff` for small Artifact comparisons.
- PTY: use `rustix` `pty` (Linux) and `windows-sys` ConPTY inside `windows-local-security` instead of adding `portable-pty`, so containment receipts come from one crate.
- Browser: implement the allowlisted CDP client in Rust on the existing `tokio` runtime; do not bundle the Python browser-harness for V1.
- UI: split `WorkbenchPanel.tsx` per view and lazy-load Browser and Terminal panes (T06, T11) to reduce the 1.05 MB bundle on the unavailable path.
- Terminal output: bounded ring buffer and cursor in Rust (T10); the UI never holds unbounded scrollback.

## Dependency order

T01 and T02 start first (T02 needs nothing). Browser lane: T01 -> T03, T04 -> T05 -> T06 (also after T02), T07. Terminal lane: T01 -> T08 -> T09 (blocked on Feature 005 Linux Environment) -> T10 -> T11 (also after T02), T13; T10 -> T12 (blocked on Feature 005 B-35). Activity: T01, T02 -> T14. Diffs: T01, T02 -> T15. Close-out: T06, T11, T13, T14, T15 -> T16 -> T17 -> T18. Owner-approved additions: T06 -> T19 (iframe guard, P-06); T15 -> T20 (worktree/VCS diff and GitHub PR ops, P-13). T18, T19, T20 -> T21 (VERIFIED depends on all).

## Cross-package dependencies

| This task | Other package task | Reason |
|---|---|---|
| T01 | Feature 005 Environment admission contract | Terminal binds to the admitted Environment identity |
| T01, T15 | Feature 003 ResourceRevision content read | Diff sources are exact ResourceRevisions |
| T05, T10 | Feature 011 authenticated IPC gate | Principal comes from the transport only |
| T07, T13 | Feature 011 Runtime drain and stop/quiescence gate | Restart and drain fencing |
| T09 | Feature 005 Linux bubblewrap Environment admission | B-23 requires an admitted Environment; B-17 Linux first |
| T12 | Feature 005 B-35 Windows Environment provider | Windows terminal stays UNAVAILABLE until then |
| T14 | Feature 001 committed events; Feature 005 Task events | Activity sources |
| T15 | Feature 005 Attempt change set; Feature 007 Artifact revisions | VCS and Artifact diffs |
| T17 | Feature 019 release matrix and packaged build; Feature 001 qualified route | Packaged E2E-08 per row |

Cross-package task numbers are named by package and outcome because those packages' TASKS.md are being renumbered in the same release-planning change.

## DeepSeek Harness proposals (PROPOSED, owner review required)

Status: PROPOSED. None of the items below is an accepted task. They come from the DeepSeek Harness studies ([desktop deep-dive](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) register A-63, proposals P-31 to P-46; [plugin deep-dive](../../docs/DEEPSEEK-HARNESS-PLUGIN-ARCHITECTURE-DEEP-DIVE-2026-11.md) register A-58). Owner review is required before any item becomes a numbered task in this package. They are labelled `PROP-Pnn` so they do not collide with the stable task IDs that `.specify/scripts/validate_feature_package.py` checks. On acceptance: assign the next free task ID in this file, copy the upstream permalink, class and reuse statement into the task, and update the [reuse map](../../docs/DEEPSEEK-HARNESS-REUSE-MAP-2026-11.md). Upstream pin `fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159` (MIT, Copyright (c) 2026 DeepSeek). No upstream code is imported (AGENTS.md section 7).

### Proposals owned elsewhere that touch this package (pointer only)

- **PROP-P45 (P-45) Default-deny webview and session permissions, including the main one, plus a guest network filter** [PROPOSED, not accepted, owner review required]: row owned by package 019; class **ADAPT**; would touch here: T06 of package 019; T04 and T06 of package 008. Upstream permalinks: [src/browser-guests.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/browser-guests.ts), [src/microphone-permissions.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/microphone-permissions.ts), [src/policy-test-auth.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/policy-test-auth.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 9 and 14 (row P-45). Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React).


## Completion evidence

For each task record in CURRENT-RUN.md and the matching docs/IMPLEMENTATION-STATUS.md row: task ID, git revision, exact command, pass/fail result with test names, OS and build (Windows x64 or Linux x64; never WSL2), harness and version where relevant (OpenCode v2.0.32), Chromium version and shell profile for provider tasks, evidence level reached (CODE, SYSTEM or USER), and reviewer. A task is DONE only when its acceptance boxes are checked against that record.
