# Errors and recovery - Workbench Browser Terminal and Computer

> **Revision 1.1: IN_REVIEW / TARGET (2026-11-12).** Actual laptop Computer view and independent local/agent observation/input replace the requested browser-only scope. Historical revision 1.0 acceptance remains below; implementation remains UNAVAILABLE. See [Computer LLD](../../../docs/COMPUTER-OBSERVATION-AND-CONTROL-LLD.md), SPEC section 20 and tasks T22..T27.

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

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

## Historical 2026-11-06 implementation gaps (superseded by SPEC current evidence)

- Current Browser iframe is not backed by browser profile or BrowserControlLease persistence; Browser policy helpers are dead-code/unwired and no routes/provider exist.
- Current Terminal is local component state with simulated command output; there is no PTY/ConPTY spawn or process owner. The arbitrary-command path fabricates exit code 0.
- Current Diffs are illustrative, not real change evidence; no Activity view is rendered.
- Computer-use provider probe returns unsupported/unavailable and all action/capture permission flags false; no backend exists.

## Canonical contract

Use existing error/event owners from `docs/SCHEMAS.md`, `docs/EVENTS.md`, `docs/schemas/error-codes.schema.json` and `docs/schemas/domain-event.schema.json`. No feature-local ErrorCode or event is introduced until the owner docs and machine schemas change together.

## Qualification gate

Exercise provider missing, profile consent, default Chrome refusal, epoch/takeover races, stale tabs, ambiguous form submit, provider crash/restart, terminal profile discovery without startup execution, PTY spawn/resize/backpressure/detach/reattach/stop, Job Object/process quiescence, shell child process, pane close and Runtime restart, diff consistency, computer permission/sensitive regions and revocation. No current end-to-end browser, terminal or computer-use USER path was executed.


## Revision 1.1 Computer / terminal recovery mapping

The Computer LLD failure matrix is the owner refinement. Existing helper refusal codes map to provider missing, lock, consent missing/revoked/expired, scope and capture/input-not-permitted. T23 promotes missing semantics (stale geometry/observation, target lost, privacy blocked, stream over-bound, transfer unknown, partial injection) into canonical errors before exposing routes; no feature-local API enum is authoritative.

Picker cancel returns idle and no authority; source removal/rotation invalidates masks and coordinate mapping; lock/secure desktop blanks all delivery and fences input. Slow consumers drop superseded frames within native/encode/transport/decode caps and surface actual quality/freshness. USER ownership does not revoke permitted local observation; agent disclosure requires its own permit throughout.

Takeover fences admission at once even when provider/capture/model work stalls. The fence is reachable from the in-app strip and, only after owner review, a qualified out-of-app global shortcut or native tray action while LiteCowork is backgrounded. If no such trigger is registered/qualified, do not admit agent computer input. Shortcut conflict/registration loss, shell/runtime incarnation change or lost control channel revokes the pending input lease; it must never reopen admission on restart. A sent click/key remains pending/ambiguous until observed; never repeat automatically. Restart invalidates private targets/handles/streams and epochs, reconciles durable intents and requires explicit source/permission revalidation. Two Workspaces cannot independently own the physical OS pointer/keyboard. Direct-native tools outside the broker remain outside this guarantee.

Terminal profile missing/replaced gives repair instead of fallback; stale writer/input sequence is rejected before stdin. Already delivered bytes and partial shell input are not undone by takeover. Return blocks on unknown foreground/input state rather than auto-entering a newline. Pane/Canvas/tab switch never respawns, terminates or silently replays a command. Manual-host execution remains an unresolved owner gate and cannot bypass B-35 agent admission.
