# UI behavior - Workbench Browser Terminal and Computer

Behavior, not pixels. Screen inventory: [V1-UI-SCREEN-CONTRACT.md](../../../docs/V1-UI-SCREEN-CONTRACT.md), [DESKTOP-SHELL-LLD.md](../../../docs/DESKTOP-SHELL-LLD.md), [CHROME-BROWSER-LLD.md](../../../docs/CHROME-BROWSER-LLD.md), and [V1-INTEGRATED-TERMINAL-LLD.md](../../../docs/V1-INTEGRATED-TERMINAL-LLD.md).

## Target surface

- Workbench is a resizable right-side view over authorized Workspace/Resource/Environment objects. It does not own privileged shell/browser processes. Closing the pane detaches the view unless the user explicitly requests a separate session Stop/Close operation.
- Files show admitted Resources or exact Environment filesystem views; labels and location identity distinguish Workspace Resource projections from live local files. A Resource preview is never labeled as a live filesystem editor.
- Browser shows provider availability, mode, engine/version, profile identity, session/tab state, current URL, permission and control owner/epoch. Managed browser and Connect Existing Chrome are distinct modes. A generic iframe preview, if retained, is labeled as embedded preview and has no agent-action or Chrome assurance badge.
- Take control/Return to agent are available only for a live provider-backed lease. UI displays the exact owner/epoch and pending-action state; Takeover disables queued agent input only after the provider confirms the new epoch; Return requires confirmation and fresh observation. Closing the side pane is separate from closing the browser or stopping a Task.
- Terminal tabs show exact shell profile/version, executable identity, current directory, Environment/process identity, containment evidence, session owner, and connection/lost state. Input/stdout/stderr and resize use a real PTY/ConPTY bridge. A tab switch, pane resize or detach/reattach does not own or restart the process.
- Explicit Stop is a separate command with process-tree/quiescence receipt. UI close is a view detach; daemon stop/restart renders ACTIVE→LOST/RECONNECTING only from authoritative service state. There is no silent shell restart.
- Diffs show actual content/revision or VCS comparison, exact source IDs/digests and true added/removed counts. Activity shows committed event/Attempt/Effect/process observations only, or is deferred to Work/Task detail by the screen contract.
- Computer use is only shown when a real provider and OS permission exists. Otherwise the UI states unavailable/unsupported with a reason; it never animates a phantom cursor or claims screen capture/input injection.

## Current implementation crosswalk

- `WorkbenchPanel` is a React right pane with launcher, Files, Terminal, Browser and Diffs paths. `WorkbenchTab` declares `activity` but no navigation or Activity render branch exists.
- Files is a list/filter of the Workspace Resource catalog; preview is fetched for an exact ResourceRevision. It is not a native filesystem tree or Environment browser.
- Browser is a generic URL input and iframe preview; default URL is the local `127.0.0.1:4242/v1/health` health endpoint. It is not a Chrome process/provider, managed profile, persistent session, browser agent or CDP/Playwright controller. Tab change unmounts its iframe; panel close and Workspace change reset state.
- Terminal has local React scrollback/input/command history only. No process spawn, shell executable, PTY/ConPTY, Tauri IPC bridge, stdin/stdout transport or daemon session exists. `help/status/dir/whoami/version` return fixed illustrative text; `dir` lists Resource records; generic commands print fabricated exit code 0. The `pwsh 7 · Windows ConPTY` and `KILL_ON_CLOSE` badges are not backed by a process or containment attestation.
- Diffs is not a VCS or exact revision diff. It marks preview lines as additions/deletions using a fixed modulo pattern; without a preview it renders a fixed sample patch and hard-coded +4/-1 stats.
- There is no Activity tab despite the `activity` union type. Environment take/return-control operations are target-only and not called by the Workbench.
- Computer-use source has policy/consent helpers only; no capture/input provider. The provider probe reports unsupported/unavailable on current platforms.

## Required rendered regression cases after implementation

- Browser provider unavailable/needs setup/offline/ready; no iframe fallback masquerades as a qualified browser session.
- Managed profile isolation and connect-existing Chrome consent/version mismatch/revoke; default Chrome profile always refused.
- Browser stale session/tab/control epoch, input in-flight takeover, provider crash, fresh-observation Return and uncertain navigation form submit.
- Terminal profile discovery must not run startup scripts; selected shell exact executable/argv/cwd/env; unsupported shell remains unavailable.
- Terminal multi-tab attach/detach, resize, backpressure/high-volume output, explicit stop, process-tree containment, owner switch, pane close vs Stop, workspace change and daemon restart.
- Actual diff with equal revisions, rename, binary/unpreviewable content, no changes, missing source and uncommitted source; never report fabricated counts.
- Computer-use permission denied/revoked, sensitive-region exclusion, provider unavailable, per-action scope and stale permission.

## Validation

Source-level policy helper tests do not qualify a browser provider, terminal process, or computer-use backend. Qualification requires the actual OS provider and USER flows on the declared GA OS/build, with event-backed receipts and crash/restart tests.
