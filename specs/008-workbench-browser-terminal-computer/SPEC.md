# 008 - Workbench Browser Terminal and Computer

**ID:** 008
**Status:** DRAFT
**Owners:** Desktop shell, Environments, Browser provider, Trust, Desktop UI
**Updated:** 2026-11-06
**Revision:** 0.3
**Target release:** V1 local desktop
**Dependencies:** root ARCHITECTURE.md, docs/V1-PRODUCT-RELEASE-CONTRACT.md, docs/IMPLEMENTATION-MASTER-PLAN.md, and the owner documents named in RESEARCH.md. Feature 001 remains the Conversation send authority.

## 1. Problem

A WebView is not Chrome. A terminal tab is not an Environment. Computer use without consent and epoch revocation is unsafe. The Workbench must show real objects only.

Observable outcome: The owner uses the right Workbench to finish real browser and terminal work, takes over and returns control, and a stale handle cannot act.

## 2. Goals

- Ship one context-aware Workbench bound to Conversation or Task.
- Qualify browser session epochs with takeover and return.
- Bind terminal PTY lifetime to an admitted Environment process tree.
- Revoke stale browser and terminal handles on epoch change or Runtime restart rules.

## 3. Non-goals

- Claiming Tauri WebView is Google Chrome.
- Headless browser pretending to be the same visible session.
- Terminal that executes shell startup scripts during discovery.
- Computer use without an explicit consent epoch.

## 4. Current evidence

The active checkout has a substantial Workbench presentation component and compiled browser-control/computer-policy primitives, but the user-facing Browser, Terminal and Diffs panes are not integrated with the intended provider/runtime owners. The current UI is materially more simulated than the normative owner LLDs. The following matrix is a source/route/UI crosswalk, not owner acceptance or release qualification.

| Capability | Current source/route evidence | Limitation / release meaning |
|---|---|---|
| Workbench shell and Files | CODE/UI: `WorkbenchPanel` is mounted as a resizable/collapsible right pane. Files view reads the current Workspace Resource catalog and exact preview selected by the Library/resource path | This is a Resource list/preview, not a native filesystem tree or Environment mount. Resource membership/content is governed by Feature 003; the Workbench does not own local filesystem authority. |
| Browser preview | CODE/UI: `WorkbenchPanel` holds a URL and renders it in an HTML `<iframe sandbox=...>` with an external-browser fallback affordance | This is an embedded WebView/iframe preview, not Chrome/Chromium provider, persistent browser session, Playwright/CDP controller or agent-controlled browser. The URL/default local health endpoint and iframe state live only in React component state. Switching tabs unmounts the iframe; closing the pane or changing Workspace remounts the component. |
| Browser consent and control policy | CODE: `browser_consent.rs` has managed-profile marker/consent/version guardrails; `browser_control_service.rs` has pure actor/lease/epoch/stale-input admission and takeover decisions with unit tests | `main.rs` explicitly compiles both modules with `allow(dead_code)` and says no Operator route, provider or Chrome process is connected. No composed browser session/command/takeover/return route or durable owner store exists. The policy helper does not dispatch or acknowledge a provider action. |
| Existing Chrome safety | CODE: default Chrome profile is refused; Connect Existing Chrome is consent/high-trust/version gated; policy candidates require a LiteCowork-managed profile and Chrome 144+ | These are policy decisions/helpers, not an attach provider or user flow. No Chrome attach/managed-launch process or visible consent/revoke UI is connected. |
| Terminal UI | CODE/UI: terminal pane has input, scrollback, help, clear/copy, command history and display labels `pwsh 7 · Windows ConPTY` / `KILL_ON_CLOSE` | Its handler executes no process. Help/status/dir/whoami/version append hard-coded sample output; every other command appends a fabricated success with exit code 0. “dir/ls” lists Workspace Resource records, not a directory. These badges and success responses are false representations of execution. |
| PTY/ConPTY lifecycle | No daemon TerminalSessionService, PTY broker, ConPTY spawn bridge, terminal Tauri command, session routes or terminal process registry was found in the active source | `windows-local-security` contains Job Object process containment primitives, but there is no Workbench terminal process using them. `WorkbenchPanel`'s local scrollback survives switching tabs only while the React component remains mounted; closing the pane unmounts it and Workspace key changes remount it. No process persists, detaches, reattaches, or survives UI unmount/restart. |
| Shell profiles and Environment binding | Owner target `V1-INTEGRATED-TERMINAL-LLD` defines shell discovery/profile selection and exact Environment binding | The current Workbench hardcodes a Windows pwsh/ConPTY label, does not discover executable profiles, does not select a shell, does not require Environment admission, and has no per-tab Environment/process identity. No startup-script-safety discovery implementation is wired. |
| Diffs/Review | CODE/UI: `WorkbenchPanel` renders a diff-like view from the currently selected Resource preview | The code marks lines by a fixed modulo pattern; with no preview it renders hard-coded sample code; selectedDiffFile is always null and summary is fixed `+4 additions / -1 deletions`. This is illustrative UI, not Git/revision diff, not an actual changed-file list and not verification evidence. |
| Activity | `WorkbenchTab` type includes `activity` | No Activity navigation button, activity branch or `setActiveTab("activity")` call exists. The final render `else` falls through to the Diffs view; no real Activity timeline is presented in this Workbench. |
| Computer use | CODE: `environment_computer_use.rs` contains consent/scope/sensitive-region policy and `probe_computer_provider()` | Module comment states no qualified capture/input-injection backend is wired and every request is refused. Probe returns Unsupported/Unavailable on current supported targets; no screen capture, input injection, window focus or desktop interaction provider/route/UI exists. |
| Environment control/takeover APIs | OpenAPI types define `takeEnvironmentControl` and `returnEnvironmentControl`; the composed route inventory classifies both as target-only | No server handler or Workbench call site. Browser policy takeover functions are pure helpers and are not connected to an EnvironmentControlLease store, provider or UI. |

Feature 008 remains DRAFT. The current Workbench is a Resource preview plus simulated terminal, generic iframe preview and illustrative diff pane. It is not a qualified native Browser, PTY/ConPTY Terminal, Activity timeline or Computer-use surface.

## 5. LOCKED and FLEXIBLE decisions

### LOCKED

- Browser provider and presentation adapter are separate (CHROME-BROWSER-LLD).
- UNAVAILABLE is not a browser mode.
- Terminal UI does not own a privileged shell (V1-INTEGRATED-TERMINAL-LLD).
- Workbench is views over authorized objects (release contract).
- Stale handles cannot act (master plan P7).

### FLEXIBLE

- Module file layout and private helper names, provided the transactions and secret boundaries stay intact.
- Presentation copy, provided state distinctions remain visible.

### Open decisions

- B-22: Which exact browser provider/distribution and visible-surface mode is first on the GA OS/build? Choose managed Chromium or consented connect-existing Chrome (not both by assumption); pin engine/channel/version, launch/profile policy, storage/retention, downloads, network restrictions, session lifecycle and the provider-to-view/presentation adapter.
- B-23: Which exact terminal provider, OS/build, shell executable/profile and Environment binding is first? Define PTY/ConPTY implementation, exact spawn argv/environment allowlist, working-directory root, Job Object/cgroup process-tree limits, resize/encoding, output/backpressure, process identity, detach/reattach, stop/quiescence and Runtime-restart semantics.
- B-32: Is Computer use in V1 or explicitly deferred? If included, select the exact native provider/platform permission flow and supported actions/capture scopes; if deferred, expose no disabled/fake Computer surface and keep the module unavailable.

### Blockers

- B-22: Desktop shell, Browser provider, Environments and Security must accept one browser provider/mode for the declared GA OS/build, including an executable versioned provider, isolated profile strategy, actual visible surface, consent/revoke, lease persistence, action dispatch, observation, takeover/return and exact stale-epoch rejection. Current browser policy code is not a provider.
- B-23: Desktop shell, Environments and Security must accept one real PTY/ConPTY implementation and shell profile, tied to a qualified Environment. Acceptance requires a real process, exact executable/argv/cwd/allowlisted env, per-session process-tree containment, bounded streaming, resize, explicit stop, detach/reattach and quiescence/lost-session proof on UI close, Runtime restart and crash.
- B-32: Product and Desktop shell must decide Computer-use scope. If same V1 milestone, Environments/Trust must supply an exact OS provider with explicit capture/input permission, scope, excluded sensitive regions, revocation and per-action authority; current code explicitly has no capture or injection backend. Otherwise defer the surface and state it as unavailable.

## 6. High-level design

Right Workbench hosts views. Browser control goes through a lease gateway outside SQLite. Terminal opens a PTY only after Environment admission. Computer actions use the same epoch and consent record. UI disablement follows provider availability.

Trust boundary: the desktop UI is untrusted; the authenticated Operator is the only command path; native credentials and OS handles stay outside public events; SQLite transactions do not perform network or process calls.

## 7. Low-level design

Takeover keeps the EnvironmentControlLease ACTIVE, moves control from AGENT epoch n to HUMAN epoch n+1, and rejects the old epoch. The browser process may stay up. Return requires explicit confirmation and another epoch increment. Handle presentation to the model is a capability grant, not the raw CDP or PTY handle in events.

Ordering: authorize, compare expected version, commit the durable receipt, then perform any external call, then reconcile. Proposed payload fields are PROPOSED until the canonical owner document and machine schema change together.

## 8. Data

See [DATA-MODEL.md](DATA-MODEL.md). This feature does not create a second source of truth beside the domain owner.

## 9. Wiring

See [WIRING.yaml](WIRING.yaml). Requests enter through the Operator. External calls sit outside transactions.

## 10. Security

Consent is explicit per epoch. Network and filesystem remain Environment policy. Screenshots and page text are untrusted input. Handles are redacted from logs.

## 11. Errors and recovery

Provider crash marks the epoch lost. Restart does not replay a half-applied browser action without reconciliation. Closing the window does not by itself kill the Runtime, but sleep and shutdown follow RUNTIME-LIFECYCLE.

Races use compare-and-swap or a single-writer occurrence identity. Timeout after an external effect reconciles. Duplicate retries return the original receipt when the idempotency key matches and fail when the payload conflicts.

## 12. Crash and restart

Reload the last committed owner records. Do not infer success from an intent. Reconcile in-flight external work before a new side effect. Cancellation fences late results.

## 13. Configuration and compatibility

Modes that are not qualified stay hidden with a repair path. Computer use can remain unavailable if B-32 explicitly defers it.

OS qualification is per native desktop. WSL2 does not qualify Windows or Linux GUI behavior.

## 14. Observability

Log ids, versions, statuses and denial reasons. Do not log secret bytes, native handles, private locators or raw credentials. Audit commits include actor, workspace and receipt id.

## 15. EARS acceptance

- **AC-001:** WHEN no qualified browser provider exists, THE SYSTEM SHALL show UNAVAILABLE and SHALL NOT label a WebView as Chrome.
- **AC-002:** WHEN the owner starts a managed browser epoch, THE SYSTEM SHALL record consent and a session epoch before any agent browser action.
- **AC-003:** WHEN the owner takes over, THE SYSTEM SHALL atomically advance the control epoch and fence/discard queued agent input; the agent SHALL perform no further browser action until a separate confirmed return to a fresh observation or the session is closed.
- **AC-004:** IF a stale epoch handle is presented, THEN THE SYSTEM SHALL reject it.
- **AC-005:** WHEN a terminal opens, THE SYSTEM SHALL spawn one real PTY/ConPTY process in the admitted Environment, persist its exact session/process identity, and SHALL NOT merge tabs or process trees across Environments.
- **AC-006:** WHEN shell profiles are discovered, THE SYSTEM SHALL perform bounded executable/version discovery without executing profile startup scripts, sourcing shell rc files, or scraping environment secrets.
- **AC-007:** WHEN the Runtime loses the provider, THE SYSTEM SHALL mark the epoch lost and reconcile in-flight actions before retry.

## 16. Executable acceptance command

```bash
python3 .specify/scripts/validate_feature_package.py specs/008-workbench-browser-terminal-computer
```

Further gates after ACCEPTED/LOCKED are the cargo filters named in TASKS.md. They are not evidence until run on a recovered source tree.

## 17. Out-of-bounds

- Do not automate the user's everyday Chrome profile by default.
- Do not treat WSL as proof of native Windows terminal qualification.

## 18. Assumptions and open questions

- Browser provider, terminal provider and Computer-use provider are separate qualification decisions. A mode/policy enum or UI control does not prove an OS process/provider exists.
- Computer use may be deferred by B-32; the current helper must remain visibly UNAVAILABLE/UNSUPPORTED rather than appear as a disabled-but-ready computer tool.
- The current diff pane is illustrative, not VCS/evidence. Activity is not a rendered Workbench tab. Whether Activity belongs in Workbench or Work/Task detail follows the canonical V1 screen contract; do not make it a fabricated fifth work surface.
- Archive UI is not served evidence.

Each blocker has the owners named above. Resolution gate: owner decision recorded in this SPEC before status leaves DRAFT. Implementation tasks: 008-T01, 008-T02, 008-T03, 008-T04.

## 7a. Owner facts and implementation crosswalk checked 2026-11-06

Read from CHROME-BROWSER-LLD, V1-INTEGRATED-TERMINAL-LLD, and STATE-MACHINES.

- AvailabilityStatus is READY, NEEDS_SETUP, UNAVAILABLE, OFFLINE, or UNKNOWN. BrowserMode is MANAGED_CHROMIUM, CONNECT_EXISTING_CHROME, MANUAL_ONLY, or HEADLESS_TASK. HEADLESS_TASK handoff to a visible browser is a new surface.
- BrowserControlOwner is AGENT, USER, NONE, or TRANSFER_PENDING. Session health is STARTING, READY, DEGRADED, DISCONNECTED, RECOVERING, or CLOSED. viewMode is LIVE_INTERACTIVE, OBSERVATION_FRAMES, EXTERNAL_WINDOW, or UNAVAILABLE. BrowserActionReceipt.state is REJECTED, ADMITTED, IN_PROGRESS, APPLIED, AMBIGUOUS, or BLOCKED. The command actor field is caller intent. Authority comes from the authenticated principal plus the control lease.
- Take control is an atomic epoch transfer. Return requires explicit confirmation, increments the epoch, and the agent replans from a fresh observation. Closing the side pane does not close the browser or cancel the Task. A matching URL after restart is not proof that a pre-crash write succeeded.
- Password fields, payments, CAPTCHA, permission prompts, and MFA require a human. Copying encrypted cookies or importing history, passwords, or login state is forbidden. Chrome remote debugging against the default user directory and Playwright automation of the default Chrome profile are forbidden.
- reconcileBrowserAction leaves UNKNOWN ambiguous and does not submit the same form twice. detachBrowserSession does not kill a user-owned Chrome profile.
- shellProcessState is STARTING, ACTIVE, EXITED, RECONNECTING, LOST, DETACHED, or STOPPING. ownerKind is USER_MANUAL, TASK_ATTEMPT, or PROVIDER_PRIVATE. An unauthorized root is rejected. Silent fallback to home is forbidden. Closing the terminal view is not Stop Task. A manual terminal history does not become Task Evidence automatically.
- OpenAPI `takeEnvironmentControl` and `returnEnvironmentControl` exist as TARGET_ONLY Task/Attempt Environment operations; they are not served and are not BrowserControlLease handlers. No browser-session or terminal-PTY operationId/route is currently served. `environment.control.lease.changed.v1` remains a target event contract; do not report it as emitted by the current Workbench.



## 7b. Current source and visible Workbench boundary — 2026-11-06

- The composed Operator has no BrowserSession create/list/get/navigate/capture/action/control-lease route, no TerminalSession/profile/PTY route and no Computer-use capture/input route. `takeEnvironmentControl` and `returnEnvironmentControl` are target-only Environment operations, not served BrowserControlLease handlers.
- `browser_consent` and `browser_control_service` compile policy helpers but are not wired to a route, browser provider or Chrome process; their call sites are tests. Consent rules refuse default Chrome profiles and require a managed-profile marker, explicit consent/high-trust acknowledgement, exact major-version match and minimum Chrome 144 for the connect-existing candidate. They do not create a session or deliver browser actions.
- `environment_computer_use` is a policy boundary only. No capture/input provider is wired; requests are refused. On Linux with a display the provider probe still reports unsupported because desktop-portal/remote-desktop paths are unqualified; non-Linux is unsupported and headless Linux is unavailable. Capture/input capability flags remain false.
- The visible Files pane lists Workspace Resource projections and exact ResourceRevision previews; it is not a native filesystem or Environment browser. The Browser pane is a sandboxed iframe with local React URL state, defaulting to the daemon health endpoint. It is an embedded preview only; tab switch unmounts it and pane/Workspace changes reset state.
- The Terminal pane uses component-local scrollback/input/history. It spawns no child process and performs no Tauri IPC. Built-in sample commands print fixed text, `dir/ls` lists Resource records, and arbitrary commands print a fabricated successful exit code without execution. The `pwsh 7 · Windows ConPTY`, `KILL_ON_CLOSE` and Job Object claims have no process/containment receipt. This is a release-blocking UI truthfulness defect, not merely an unfinished backend.
- Diffs has no selected source file, labels lines through a fixed modulo pattern, and otherwise renders a static sample patch with hard-coded `+4/-1` counts. It is not a VCS or exact ResourceRevision diff. `WorkbenchTab` includes `activity` but no navigation or Activity render branch exists; it falls through to Diffs. Computer use has no live UI surface.
- `windows-local-security` Job Object helpers are not a PTY/ConPTY provider and do not prove a terminal process was admitted to an Environment. No terminal session store, shell-profile registry, process spawn, resize/stdin/stdout bridge or explicit Stop operation was found.
- The current Workbench right pane is keyed by Workspace and unmounts on close. Its view-local browser and terminal state is therefore lost on pane close or Workspace change. This is distinct from the target contract where detach/reattach must not own or restart a provider process.
- The UI contract and RESEARCH.md contain the exact current source crosswalk. B-22 (browser provider/mode), B-23 (PTY/ConPTY and shell profile) and B-32 (Computer-use include/defer and OS provider) remain open. Until a real process is wired, remove/disable synthetic command execution and ungrounded success/containment claims.

## 19. Requirement traceability

| Requirement | Spec | Canonical contract | Task | Acceptance |
|---|---|---|---|---|
| AC-001 | EARS acceptance | canonical owner docs cited in this SPEC | 008-T01 | AC-001 |
| AC-002 | EARS acceptance | canonical owner docs cited in this SPEC | 008-T01 | AC-002 |
| AC-003 | EARS acceptance | canonical owner docs cited in this SPEC | 008-T02 | AC-003 |
| AC-004 | EARS acceptance | canonical owner docs cited in this SPEC | 008-T02 | AC-004 |
| AC-005 | EARS acceptance | canonical owner docs cited in this SPEC | 008-T03 | AC-005 |
| AC-006 | EARS acceptance | canonical owner docs cited in this SPEC | 008-T03 | AC-006 |
| AC-007 | EARS acceptance | canonical owner docs cited in this SPEC | 008-T02 | AC-007 |
