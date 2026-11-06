# Research - Workbench Browser Terminal and Computer

**Feature:** 008 | **Checked:** 2026-11-06 (active source, route composition, Tauri bridge and visible Workbench UI)

## Sources inspected

- Normative owner LLDs: `docs/CHROME-BROWSER-LLD.md`, `docs/V1-INTEGRATED-TERMINAL-LLD.md`, `docs/ENVIRONMENTS.md`, `docs/RUNTIME-LIFECYCLE.md`, `docs/TRUST.md`, `docs/TASK-RUNTIME.md`, `docs/DESKTOP-SHELL-LLD.md`, `docs/V1-UI-SCREEN-CONTRACT.md`.
- Daemon: `apps/litecoworkd/src/main.rs`, `browser_consent.rs`, `browser_control_service.rs`, `environment_computer_use.rs`, `environment_action_batch.rs`, `environment_provider.rs`, `environment_isolation.rs`, `operator.rs` and the composed `docs/schemas/served-route-inventory.json`.
- Desktop UI/native bridge: `apps/litecowork-ui/src/workbench/WorkbenchPanel.tsx`, `workbench-panel.css`, `apps/litecowork-ui/src/App.tsx`, `apps/litecowork-ui/src-tauri/src/lib.rs`, Tauri source modules and generated `operator-api.ts`.
- Containment primitives: `crates/windows-local-security`, process-scope and environment crates. Search for ConPTY/PTY providers, terminal spawn/session routes and provider consumers was run across `apps/` and `crates/`.

## Composed route and provider findings

- The composed route inventory has no served BrowserSession create/list/get/navigate/capture/action/control-lease route, no TerminalSession/profile/PTY route, and no Computer-use capture/input route. The OpenAPI `takeEnvironmentControl` and `returnEnvironmentControl` operations, plus Environment provision/lifecycle operations, are target-only.
- `main.rs` contains explicit comments on `browser_consent` and `browser_control_service`: compiled but not wired because no Operator route, provider or Chrome process is connected. `environment_computer_use` is also marked `allow(dead_code)`; it is a policy boundary, not a provider.
- `browser_control_service.rs` implements pure policy/admission helpers for authenticated actor, owner, session, expected epoch, session revision, tab generation, in-flight inputs and takeover/return. Its module comment says it decides/records intent only and does not dispatch provider input. The only call sites found are its own unit tests.
- `browser_consent.rs` includes safety rules: it refuses default Chrome profiles, requires a LiteCowork-managed profile marker, consent, high-trust acknowledgement, exact browser major-version match and minimum Chrome 144 for the connect-existing candidate. These helpers do not launch a browser, attach to Chrome, persist active BrowserSessions, create a provider surface or deliver page actions. No production Chrome process consumer was found.
- `environment_computer_use.rs` explicitly says no qualified capture/input backend is wired and every request is refused. `probe_computer_provider()` returns `UNSUPPORTED_PLATFORM` on non-Linux; on Linux with a display it still reports UNSUPPORTED because desktop-portal capture/remote-desktop paths are unqualified; without a display it reports UNAVAILABLE. Capture and input-injection capability flags are false.
- `windows-local-security` has Windows Job Object process-tree containment helpers. That is not a PTY/ConPTY provider. Repository search found no `CreatePseudoConsole`, `openpty`, `portable_pty`, PTY broker, terminal-session store, daemon terminal command, or Tauri terminal spawn/resize/stdin bridge. The only “ConPTY” mentions in the visible Workbench are display text and initial sample output.

## Workbench React UI findings

- `WorkbenchPanel` is mounted by `App.tsx` as a right-pane component and is keyed by `selectedWorkspaceId`. It is resizable/collapsible and the Workbench view is conditionally mounted; closing it unmounts the component, and changing Workspace remounts it.
- The Files view is a filtered list of Workspace Resource projections. Clicking a row loads an exact ResourceRevision text preview through App callbacks. It does not read a live native filesystem tree or mount an Environment filesystem. File names and uploaded Resources are not proof of current disk contents.
- Browser uses `browserUrl`/`browserInputUrl` in React state and an HTML iframe with a sandbox attribute. Its default URL is `http://127.0.0.1:4242/v1/health`. This is a generic embedded WebView preview, not a Chrome session or managed browser. It has no provider tab/session identity, Chrome executable/version, managed profile, consent, CDP/Playwright transport, action lease, screenshot/DOM observation or history persistence. When switching to another tab, the conditional removes the iframe; returning creates a new iframe. Closing the pane or changing Workspace resets the component state.
- Terminal uses component-local `terminalScrollback`, `terminalInput`, `commandHistory` and `historyIndex`. `handleRunTerminalCommand` launches no child process and performs no IPC. `help`, `status`, `dir/ls`, `whoami` and `version` append hard-coded sample lines; `dir/ls` enumerates Resource records, not an OS directory; arbitrary commands append a fabricated “completed with exit code 0” message. The header and initial scrollback claim `pwsh 7 · Windows ConPTY`, `KILL_ON_CLOSE` and Job Object resource enforcement without a PTY process. The local history survives tab switches only because WorkbenchPanel remains mounted; it is not a resident shell. Closing the pane/Workspace switch loses it.
- Diffs has no selected source file (`selectedDiffFile` is always null). If a Resource preview exists, the code labels lines as additions/deletions on a fixed modulo pattern, not by computing a comparison. Without preview it renders a static example patch, and the summary is hard-coded `+4 additions / -1 deletions`. This is not a VCS diff, Resource revision diff or proof of changed files.
- `WorkbenchTab` declares `activity`, but no Activity navigation control, `setActiveTab("activity")` call or Activity render branch was found. The terminal branch is separate, Browser branch is separate, and final `else` renders Diffs. Activity is not a usable Workbench tab in this component.
- Computer controls do not appear as a live Workbench surface. The UI has no screen capture/input control provider path, and the backend probe cannot return READY on the current qualified platform set.

## Product viability assessment

- The shell/side-pane layout is a useful container, but the current Browser, Terminal and Diff views should not be presented as working automation/execution tools. The synthetic terminal success is a release-blocking truthfulness defect: it tells a user that arbitrary shell commands completed successfully when no process ran. The sample containment and memory-quota output is likewise unsupported.
- The current iframe can be retained as an explicitly named “embedded preview” mode, but it must not satisfy the Browser provider/control contract or be described as a managed Chrome session. The real provider must own profile, session, tabs, observations, consent and action execution. The display surface is a separate adapter.
- The Terminal pane needs a real daemon-owned PTY/ConPTY session service attached to the exact admitted Environment. UI mount/unmount and tab selection must not own the child process; close, detach, stop, Runtime restart and Workspace change need distinct lifecycle semantics. A Job Object badge without a process attached to it is not containment evidence.
- Diff and Activity must consume actual domain events, persisted Attempt/Effect/Artifact records, immutable Resource revisions or VCS changes. If the Activity view is moved to Work/Task detail by the screen contract, the Workbench should not retain a fake `activity` enum.
- Computer use is separately deferred/blocked until the OS permission provider and actual capture/input injection backend are qualified. Consent/policy data models alone cannot enable it.

## Alternatives and decisions

- Call the current iframe a full Chrome/browser automation surface: rejected. It is a generic embedded preview only.
- Treat local React scrollback as resident PTY: rejected. No process/session is created.
- Claim Job Object/KILL_ON_CLOSE based on the UI badge: rejected unless an actual terminal process is attached and exact containment is attested.
- Preserve fake terminal success for design polish: rejected. Unsupported or unavailable must be visible, and arbitrary commands must never return fabricated exit code 0.
- Build a separate browser/terminal security engine in the UI: rejected. Provider processes and actions belong behind daemon/Environment/Trust owners.

## Unresolved decisions

See SPEC §5 blockers B-22 (exact browser provider/mode and real session surface), B-23 (exact PTY/ConPTY provider, shell profile and Environment/process lifecycle), and B-32 (include or defer Computer use and exact OS provider/permission contract).
