# Data model - Workbench Browser Terminal and Computer

**Feature:** 008 | **Status:** DRAFT

Identities and lifecycles stay in the canonical owner documents. This file lists the records this slice reads or commits. It does not authorize new tables by itself.

## Records

- Browser session epoch and consent.
- Terminal session bound to Environment id.
- Redacted handle refs.
- AvailabilityStatus.

## Relations

Workspace is the security boundary. Child records carry workspace id. Runtime-local secrets, handles and private locators are excluded from replicated events and ordinary backups.

## Lifecycle

Create and settle through the transactions in WIRING.yaml. External effects happen after commit. Terminal states are explicit. Unknown is not success.

## Persistence and events

Use existing event names in [docs/EVENTS.md](../../docs/EVENTS.md). A new event type requires a same-change edit to EVENTS.md and `docs/schemas/domain-event.schema.json`.

## Retention

Follow the owner retention rules. Purge removes future visibility of payload bytes. Receipts of the deletion may remain.

## Migration

None in this revision. Blockers in SPEC section 5 must be closed before a migration is designed.

## Current implementation boundary (2026-11-06)

- The current React Workbench is a view component. It is not the owner of BrowserSession, TerminalSession, EnvironmentControlLease, process identity, browser profile or consent.
- No active persisted BrowserSession/BrowserControlLease/TerminalSession owner record or production Operator route was found. Browser policy helper structs and target data names are not evidence that these owner records are persisted.
- Browser control epoch/consent and terminal shell/process fields remain target records governed by `docs/CHROME-BROWSER-LLD.md`, `docs/V1-INTEGRATED-TERMINAL-LLD.md`, `docs/ENVIRONMENTS.md` and `docs/TRUST.md`.
- Runtime-local browser CDP handles, Chrome profile paths, PTY handles, OS process handles, shell environment values and raw terminal bytes must never enter replicated domain events or public projections. User-facing projections expose opaque session/process references and redacted health only.
- Current UI-local terminal scrollback and browser URL do not have durable owner identity or replay semantics; they must not be treated as session records or Task Evidence.
- Job Object process containment records from `windows-local-security` are separate OS primitives. They become TerminalSession evidence only when the actual spawned shell process is attached and measured under the admitted Environment.
- No database migration is authorized in this DRAFT. A future persistent session/lease schema requires owner decision, event/schema promotion and explicit migration/recovery design.
## Current implementation boundary checked 2026-11-06

- Workspace Resources and ResourceRevisions remain owned by Feature 003/ResourceStore. The Workbench Files pane consumes a Workspace-scoped Resource catalog projection and fetches exact ResourceRevision content for preview; it is not a native filesystem tree, Environment mount or shell working directory.
- `WorkbenchPanel` local React state currently owns iframe URL, selected tab, terminal scrollback/input/history and diff selection. These values are not durable BrowserSession, TerminalSession, BrowserControlLease or VCS/Artifact/Resource change records.
- BrowserMode, ChromeAttachConsent, BrowserControlLease and BrowserActionReceipt are target owner concepts. Current `browser_consent.rs` and `browser_control_service.rs` types/helpers are compiled but dead-code/unwired; no active Operator store writes a BrowserSession or control lease and no Chrome process consumes them.
- ComputerObservationConsent, ComputerObservationScope and ComputerAction policy values are compiled in `environment_computer_use.rs`; current provider availability is unsupported/unavailable and no capture/input provider or route exists.
- TerminalSession, shell profile identity, PTY/ConPTY process identity, output cursor, detach/reattach token and quiescence proof are target owner concepts. No current daemon record/store, PTY process, terminal session route or Tauri PTY bridge exists.
- Current iframe/terminal/diff state is ephemeral component state. Unmounting WorkbenchPanel discards it; changing Workspace remounts it. This is not persistence or restart recovery.
- EnvironmentControlLease `takeEnvironmentControl`/`returnEnvironmentControl` are OpenAPI target-only Task/Attempt operations; they are not browser-control APIs. `environment.control.lease.changed.v1` is a target event and is not emitted by the current Workbench.