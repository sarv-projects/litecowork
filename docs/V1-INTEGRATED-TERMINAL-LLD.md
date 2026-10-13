# V1 Integrated Terminal, Shell Discovery, PTY and Agent Environment LLD

Status: normative TARGET, not a shipped terminal implementation. Authority: ENVIRONMENTS, RUNTIME-LIFECYCLE, TRUST, TASK-RUNTIME, CAPABILITY-INVOCATIONS, ARTIFACTS-EVIDENCE, DESKTOP-SHELL-LLD. The right Workbench hosts the terminal UI; it does not own a privileged shell. Native terminals and external native agent CLIs retain their own behavior.

## Product experience and shell profiles

A terminal can be opened from Workbench Start, Task/Environment inspector or explicit user action. Workspace default, current working directory, shell and Runtime are visible. The right pane may hold multiple terminals/tabs; clicking a tab does not merge its process with a different Environment. A detachable/maximizable terminal is a view mode change, not process relocation.

Discover, show and let user select qualified shell profiles:
- Windows: PowerShell 7 (pwsh), Windows PowerShell, cmd.exe, installed WSL distributions via wsl.exe with exact distro, Git Bash, optional trusted MSYS2/Cygwin shells.
- Linux: bash, zsh, fish, sh/dash, installed pwsh and project-qualified container shell via EnvironmentProvider.
- macOS: zsh, bash, fish, installed pwsh and qualified login shell.
- Custom: user-picked exact executable+fixed argv/profile/env, validated as an executable under an approved Runtime. Shell detection should not execute startup scripts, scrape secrets or run a shell command whose path came from arbitrary web/provider metadata.

Profiles carry id/display/OS/Runtime/shell kind/version/path identity/default cwd policy/fixed args/environment variable allowlist/trust category/shell integration capability. Hide Unsupported/Unavailable with reason rather than offering an inert terminal. Use system preferred shell only when explicitly resolved to an executable for that Runtime, never assume Linux bash exists on Windows native Runtime.

## Architecture

~~~text
React RightWorkbench TerminalSurface
  xterm-compatible renderer, copy/select/find/resize/accessible text view
 -> authenticated local Tauri IPC Terminal bridge
 -> litecoworkd TerminalSessionService
      -> TerminalProfileRegistry / OS discovery broker
      -> EnvironmentProvider (local OS, WSL, container, Task Environment)
      -> TrustService + Task/Attempt/Effect admission
      -> PTY broker (Linux/macOS PTY; Windows ConPTY or qualified PTY impl)
      -> controlled shell process in exact Runtime/Environment
      -> session output framing + bounded scrollback/checkpoints
      -> terminal health/reconnection owner + diagnostic observation
~~~

Only one owning broker may interact with each PTY stdin/resize. React cannot spawn arbitrary child processes or talk to native PTY from WebView. Task/Attempt shell commands are admitted through the same environment, Trust and Effect authorities as nonterminal actions; an interactive manual terminal is not automatically a Task Attempt. Never reuse a manual terminal's credentials/working directory to grant an agent implicit execution authority. Shell integration is a trusted optional process capability, not a security/approval bypass.

## Typed targets, not existing Operator API

~~~ts
type TerminalProfile = {
 profileId:string; runtimeId:string; shellKind:string; displayName:string;
 executableRef:string; argv:string[]; available:boolean; version?:string;
 defaultCwdPolicy:"WORKSPACE_ROOT"|"TASK_WORKTREE"|"ENVIRONMENT"|"HOME"|"CUSTOM";
 supportedIntegration:"NONE"|"BASIC"|"RICH"; source:"DETECTED"|"USER_DEFINED";
};
type TerminalSessionProjection = {
 terminalSessionId:string; terminalIncarnationId:string; runtimeId:string;
 environmentId:string; workspaceId:string; taskId?:string; attemptId?:string;
 ownerKind:"USER_MANUAL"|"TASK_ATTEMPT"|"PROVIDER_PRIVATE";
 profileId:string; cwdDisplay:string; shellProcessState:
 "STARTING"|"ACTIVE"|"EXITED"|"RECONNECTING"|"LOST"|"DETACHED"|"STOPPING";
 shellIntegration:"NONE"|"BASIC"|"RICH";
 sessionRevision:number; streamCursor:string; lastOutputAt?:string;
};
type TerminalOutputFrame = {
 terminalSessionId:string; incarnationId:string; sequence:number;
 kind:"DATA"|"RESIZE_ACK"|"EXIT"|"HEALTH"|"GAP";
 sanitizedPreview?:string; bytesRef?:string; monotonicAgeMs?:number;
};
~~~

No opaque OS handle, PTY fd, exec token, secret environment variable, raw passwords or unredacted terminal transcript in domain Event/UI telemetry. Exact shell argv is controlled by trusted profile; do not run through an extra shell simply to launch a shell. CWD must resolve inside user-chosen Workspace root or authorized Environment; symlink escape is revalidated at admission.

## UI behavior

Top bar: terminal dropdown shell profile, Runtime/Environment badge, New Terminal, Split Terminal, Rename, Search, Copy output, Clear visible screen, scrollback limit, Settings, Detach/Close. Terminal tabs show process/exit status and exact profile; keyboard shortcuts configurable to avoid collisions with global and native composer commands. Drag splitter accessible, preserve chat input/unsent draft. Terminal right pane geometry follows DESKTOP-SHELL-LLD; multiple splits use bounded count and minimum width, avoid thrashing expensive renderer.

Default modes:
- Manual terminal: user can type into their own shell with explicit environment context; LiteCowork displays permission/path context. Its history is not automatic Task Evidence.
- Agent-owned terminal: read-only by default while agent owns its Attempt/control lease. Take control requires explicit epoch handoff and fences agent input, subject to OS/Environment qualification. User returning control requires fresh observation/permissions; don't replay outdated pending commands.
- Provider-private terminal: a native agent can own its CLI UI/process; only show via a qualified native adapter when allowed. Do not hijack the upstream harness's private TTY or config.

Native-agent-private shell activity not admitted through LiteCowork's TerminalSessionService/EnvironmentProvider cannot be fenced or guaranteed observed by this PTY control lease. Such an upstream terminal may be shown only through a qualified native harness session/stream with truthful limited-control status; no pretend Take Control button or complete audit proof. A terminal should not disappear when switching to Browser or Artifact. Closing its VIEW keeps process running only if user opted for keep-alive and provider/Runtime permits it; Closing Process sends reviewed graceful termination and reports PENDING while active Effects may reconcile. Terminal panel close never equals Stop Task.

## PTY and stream protocol

- Select a trusted OS-specific PTY backend: Linux/macOS PTY; Windows ConPTY. A shell profile may invoke an authorized WSL distribution as a child of Windows Runtime; once inside WSL show its own Environment identity. Local containers/remote sessions require qualified EnvironmentProvider and are not assumed present in V1 remote cloud.
- Spawn with argv array, exact executable, bounded environment allowlist/denylist, user identity from authenticated Runtime, scoped cwd and process-group ownership. No arbitrary shell concatenation or hidden startup flags from registry feeds.
- PTY wire frames are bounded, multiplexed by exact session/incarnation and monotonic output sequence. Resize events versioned and clamped; flow control and bounded ring/scrollback handles slow React renderer without blocking PTY reader indefinitely. Gaps surface warning and reset expected cursor, not falsely complete command transcript.
- On reconnect, reattach only when PTY and process are still live and same Runtime incarnation/ownership. Stream replay from in-memory ring/storage checkpoint cursor, no process restart pretending resume. If OS/process restarted, show LOST or separately launch a NEW terminal with explicit consent. Command replay is never automatic.
- Terminal output is untrusted. Escape parser must limit OSC/CSI/DCS sequence lengths, sanitize hyperlinks and URL schemes, prevent arbitrary clipboard writes/window-title control and filesystem operations unless user confirms. Do not allow OSC sequences to bypass app Trust/UI.
- IME, Unicode and accessibility require real Windows/macOS/Linux checks. Handle 100/200/400% zoom, high contrast, redraw/resizing and paste confirmation for multiline/large or secret-looking content. Alternative accessible text/log mode with explicit gaps.

## Shell integration (VS Code pattern without blind adoption)

Shell-integration quality: NONE/BASIC/RICH. When opt-in and supported by exact shell, known prompt markers can expose current cwd, command start/end, exit code and command output range. Prefer vendor/documented OSC 633 and terminal integration prompts; do not parse arbitrary prompt decorations as a trusted command audit. A malicious process can print terminal escape markers: they are UI hints, NEVER proof of Task/Effect success or permission. Authenticated Provider action receipts and OS exit status remain separate.

Supported UI extras when qualified: command decorations with exit status; navigate between commands; rerun manually with approval and current Context; copy command/cwd; search output; link open with confirmation; cwd-based file picker; command duration/previous activity overview. If no shell integration, terminal still works and shows NONE instead of guessed exit success. Do not automatically inject shell startup scripts into a user's config. A shell change creates a new TerminalSession; never mutate an active AgentSession.

## Security and lifecycle scenarios

| Scenario | Owner gate | Required response |
|---|---|---|
| User selects custom shell path | trusted picker + OS execute permission + scope | validation, version/probe, no automatic PATH mutation |
| Task issues terminal command | Task/Attempt/Effect+Environment lease | attempt and effect identity; approval per trust policy |
| Output contains secrets/URLs | output sanitization and privacy policy | no logs/telemetry leak; opt-in limited scrollback persistence |
| Browser/terminal takeover | control lease epoch CAS | old agent PTY input fenced, no mixed actors |
| WSL distro terminated | Environment/Runtime liveness | terminal LOST, recovery separate, no fake resume |
| App window closed | Runtime retention policy | view detaches; user sees whether process remains |
| Operation kills shell | process owner group and active effects | graceful stop, then forced only with explicit impact; task not marked failed by UI alone |
| User types untrusted paste | interactive prompt | prompt for multiline/control characters, no blind execution |
| Terminal opened in unauthorized root | Workspace/trust realpath | reject path, show repair; never fallback to home silently |
| Suspicious escape/OSC sequence | PTY renderer sanitizer | discard unsafe control, no clipboard or native action |

## Proposed functions

detectShellProfiles(runtime, os) -> bounded profile list / availability;
validateShellProfile(profile, runtime, trustedOwner) -> allow/reject/version;
createTerminalSession(profileId, workspace, env, purpose, requestId) -> authenticated receipt and session projection;
attachTerminalSession(sessionId, incarnation, scope) -> cursor and authorized stream;
subscribeTerminalFrames(sessionId, cursor, epoch) -> bounded ordered frames;
sendTerminalInput(sessionId, controlEpoch, bytes, authorization) -> input receipt or DENIED;
resizeTerminal(sessionId, cols, rows, revision) -> resize acknowledged;
requestTerminalTakeover(sessionId, owner, revision) -> fenced lease epoch or blocker;
requestTerminalReturnControl(sessionId, owner, revision) -> new agent lease after state refresh;
stopTerminalProcess(sessionId, mode, impactApproval) -> requested drain/exit/UNKNOWN;
clearTerminalViewport(sessionId) -> local viewport only; no audit deletion;
detectShellIntegration(sessionId, trustedProfile) -> NONE/BASIC/RICH;
deriveCommandMarks(ptyFrame, integrationQuality) -> display hints, NEVER authorization;
exportTerminalOutput(sessionId, owner, redactionMode) -> bounded reviewed local export;
observeTerminalHealth(sessionId) -> live process, PTY queue, dropped-frame and failure counters.

### Tests

Windows pwsh, Windows PowerShell, cmd, Git Bash, WSL; Linux bash, zsh, fish; macOS zsh/bash and unsupported profile fallback. Browser pane switch preserves PTY and typed data. Child process crashes while daemon lives: exact terminal LOST and no old cursor merged. Same session connected to two windows: one input owner/epoch, both authorized read-only views. OSC clipboard/hyperlink injection rejected. Multi-line paste prompt, Unicode/IME, 200% zoom, high-contrast, screen reader. Provider Task command follows Effect approval and duplicate prevention; plain user's commands do not silently become Task Evidence. Storage pressure bounds scrollback without deleting audit. App close vs Task stop semantics remain distinct.

## External source

VS Code terminal profiles: https://code.visualstudio.com/docs/terminal/profiles
VS Code shell integration: https://code.visualstudio.com/docs/terminal/shell-integration
VS Code persistent sessions: https://code.visualstudio.com/docs/terminal/advanced

Borrow profile and rich command UX patterns; do not copy VS Code's Electron/Node PTY process architecture blindly into Tauri Rust or promise process revive after a real Runtime crash. Implementation may select an xterm-compatible renderer and OS PTY broker only after security/conformance tests.
