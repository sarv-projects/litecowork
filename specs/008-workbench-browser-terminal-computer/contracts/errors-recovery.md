# Errors and recovery - Workbench Browser Terminal and Computer

## Target Browser recovery

- Browser provider or profile unavailable is an explicit `UNAVAILABLE`, `NEEDS_SETUP`, `OFFLINE` or `UNKNOWN` observation, not a ready session. Do not fall back to the Tauri WebView and call it Chrome.
- A BrowserControlLease is bound to the exact Workspace/Environment/Runtime incarnation/session/tab generation. Expiry, session revision mismatch, stale epoch, wrong actor, stale tab, provider loss or in-flight inputs reject the action before provider dispatch.
- Takeover cannot increment epoch until queued/in-flight agent inputs are cancelled or positively drained. An ambiguous navigation/form submit is reconciled against exact provider observation; never replay a stale selector or payment action.
- Return requires new owner confirmation, current provider identity/lease and fresh DOM/accessibility/screenshot/form state. Browser process restart is a new provider session and does not inherit action success from matching URL.
- Consent revocation blocks further capture/input and detaches an existing Chrome session without killing a user-owned browser.

## Target Terminal recovery

- Shell discovery runs bounded executable/version checks only; it does not execute startup scripts, source profiles or scrape secrets. Unknown profile identity or unsupported executable is unavailable.
- PTY/ConPTY spawn failure leaves no ACTIVE session. Duplicate create request replays the exact session receipt; stale or changed request identity conflicts.
- PTY output is bounded and flow-controlled. On renderer detach, the daemon-owned process remains alive only if policy and explicit session lifetime permit; output is retained within the declared bound. Attach requires matching Workspace, Runtime incarnation, session ID and owner scope.
- Explicit Stop terminates only the exact owned process tree and requires QUIESCENT evidence. If process/job status cannot be observed, report STOPPING/UNKNOWN/LOST; do not claim process exited.
- Runtime restart does not silently rerun the command. Old sessions become LOST unless the exact process/PTY survives and is re-observed under the same Runtime incarnation and lease.

## Target Computer-use recovery

- Missing OS permission, display server, capture/input provider, unsupported platform or revoked consent blocks the action before input/capture. Sensitive regions remain excluded.
- Provider crash or lost acknowledgement yields UNKNOWN/AMBIGUOUS until exact process/window observation reconciles the action. Never retry irreversible clicks/submit/payment automatically.

## Current implementation gaps

- Current Browser iframe is not backed by browser profile or BrowserControlLease persistence; Browser policy helpers are dead-code/unwired and no routes/provider exist.
- Current Terminal is local component state with simulated command output; there is no PTY/ConPTY spawn or process owner. The arbitrary-command path fabricates exit code 0.
- Current Diffs are illustrative, not real change evidence; no Activity view is rendered.
- Computer-use provider probe returns unsupported/unavailable and all action/capture permission flags false; no backend exists.

## Canonical contract

Use existing error/event owners from `docs/SCHEMAS.md`, `docs/EVENTS.md`, `docs/schemas/error-codes.schema.json` and `docs/schemas/domain-event.schema.json`. No feature-local ErrorCode or event is introduced until the owner docs and machine schemas change together.

## Qualification gate

Exercise provider missing, profile consent, default Chrome refusal, epoch/takeover races, stale tabs, ambiguous form submit, provider crash/restart, terminal profile discovery without startup execution, PTY spawn/resize/backpressure/detach/reattach/stop, Job Object/process quiescence, shell child process, pane close and Runtime restart, diff consistency, computer permission/sensitive regions and revocation. No current end-to-end browser, terminal or computer-use USER path was executed.
