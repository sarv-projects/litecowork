# Data model - Workbench Browser Terminal and Computer

> **Revision 1.1 scope revision (2026-11-12): IN_REVIEW / TARGET.** The user explicitly requested whole-laptop Computer observation and immediate manual takeover, rejecting the earlier browser-only substitution. Revision 1.0 owner acceptance (2026-11-09) and B-32 deferral below are historical authority for that revision; they are not acceptance of this extension. Implementation remains unchanged and Computer remains UNAVAILABLE. The revised owner design is [Computer observation and control LLD](../../docs/COMPUTER-OBSERVATION-AND-CONTROL-LLD.md). No new provider, transport, route or qualification is approved by this documentation pass.

**Feature:** 008 | **Status:** IN_REVIEW (revision 1.1); revision 1.0 ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.

Identities and lifecycles stay in the canonical owner documents. This file lists the records this slice reads or commits. It does not authorize new tables by itself.

## Records

- Browser session epoch and consent.
- Terminal session bound to Environment id.
- Redacted handle refs.
- AvailabilityStatus.
- Ordered terminal CommandReceipt and QuiescenceReceipt (B-23).
- WorkbenchActivityItem read projection over committed events and receipts (no new table).
- RevisionDiff result computed on demand from exact ResourceRevision or VCS identities (no new table; not cached as evidence).

## Relations

Workspace is the security boundary. Child records carry workspace id. Runtime-local secrets, handles and private locators are excluded from replicated events and ordinary backups.

## Lifecycle

Create and settle through the transactions in WIRING.yaml. External effects happen after commit. Terminal states are explicit. Unknown is not success.

## Persistence and events

Use existing event names in [docs/EVENTS.md](../../docs/EVENTS.md). A new event type requires a same-change edit to EVENTS.md and `docs/schemas/domain-event.schema.json`.

## Retention

Follow the owner retention rules. Purge removes future visibility of payload bytes. Receipts of the deletion may remain.

## Migration

The blockers are ACCEPTED (2026-11-09). One new immutable SQLite migration is authorized for BrowserSession, BrowserControlLease, ChromeAttachConsent/epoch, BrowserActionReceipt, TerminalSession, CommandReceipt and QuiescenceReceipt. It is designed and promoted by 008-T01 (allocated `sqlite-v41.sql`) and implemented by T03 and T08. No migration exists yet.

## Current implementation boundary (2026-11-06)

- The current React Workbench is a view component. It is not the owner of BrowserSession, TerminalSession, EnvironmentControlLease, process identity, browser profile or consent.
- No active persisted BrowserSession/BrowserControlLease/TerminalSession owner record or production Operator route was found. Browser policy helper structs and target data names are not evidence that these owner records are persisted.
- Browser control epoch/consent and terminal shell/process fields remain target records governed by `docs/CHROME-BROWSER-LLD.md`, `docs/V1-INTEGRATED-TERMINAL-LLD.md`, `docs/ENVIRONMENTS.md` and `docs/TRUST.md`.
- Runtime-local browser CDP handles, Chrome profile paths, PTY handles, OS process handles, shell environment values and raw terminal bytes must never enter replicated domain events or public projections. User-facing projections expose opaque session/process references and redacted health only.
- Current UI-local terminal scrollback and browser URL do not have durable owner identity or replay semantics; they must not be treated as session records or Task Evidence.
- Job Object process containment records from `windows-local-security` are separate OS primitives. They become TerminalSession evidence only when the actual spawned shell process is attached and measured under the admitted Environment.
- No database migration exists yet. The accepted persistent session/lease schema still requires event/schema promotion and explicit migration/recovery design in the T01 change.
## Current implementation boundary checked 2026-11-06

- Workspace Resources and ResourceRevisions remain owned by Feature 003/ResourceStore. The Workbench Files pane consumes a Workspace-scoped Resource catalog projection and fetches exact ResourceRevision content for preview; it is not a native filesystem tree, Environment mount or shell working directory.
- `WorkbenchPanel` local React state currently owns iframe URL, selected tab, terminal scrollback/input/history and diff selection. These values are not durable BrowserSession, TerminalSession, BrowserControlLease or VCS/Artifact/Resource change records.
- BrowserMode, ChromeAttachConsent, BrowserControlLease and BrowserActionReceipt are target owner concepts. Current `browser_consent.rs` and `browser_control_service.rs` types/helpers are compiled but dead-code/unwired; no active Operator store writes a BrowserSession or control lease and no Chrome process consumes them.
- ComputerObservationConsent, ComputerObservationScope and ComputerAction policy values are compiled in `environment_computer_use.rs`; current provider availability is unsupported/unavailable and no capture/input provider or route exists.
- TerminalSession, shell profile identity, PTY/ConPTY process identity, output cursor, detach/reattach token and quiescence proof are target owner concepts. No current daemon record/store, PTY process, terminal session route or Tauri PTY bridge exists.
- Current iframe/terminal/diff state is ephemeral component state. Unmounting WorkbenchPanel discards it; changing Workspace remounts it. This is not persistence or restart recovery.
- EnvironmentControlLease `takeEnvironmentControl`/`returnEnvironmentControl` are OpenAPI target-only Task/Attempt operations; they are not browser-control APIs. `environment.control.lease.changed.v1` is a target event and is not emitted by the current Workbench.

## Revision 1.1 record/lifecycle mapping (IN_REVIEW / TARGET)

The Computer LLD is the detailed owner refinement; do not create a second control lease. Proposed durable metadata: ComputerSession (Workspace, Runtime/RuntimeIncarnation, opaque selected target, capture generation, session revision, health, privacy-policy revision); independent local-view and agent-observation permits (authenticated audience/AgentSession, scope digest, expiry/revocation); reference to existing EnvironmentControlLease and transfer state; idempotent action intent/outcome and stop/purge receipts. Task/Attempt binds only agent mutation; ordinary local preview is not a Task. Observation-only attachment identity and Conversation vision authorization require T22 owner resolution.

Private incarnation state: OS target handles, permission bindings, capture buffers, encoder/decode queues, stream nonce, foreground/geometry map, pressed synthetic keys and physical-input detector. Never replicate/back up/log these values. Ephemeral frame metadata carries session/incarnation/capture generation/sequence/geometry/redaction revisions and age; pixels are neither a domain event nor a recording. Native display/window labels may be sensitive and are minimized. Explicit saved snapshots use existing reviewed ArtifactVersion/digest/retention, not a new screenshot store.

Lifecycle separates observation STARTING/LIVE/PAUSED/LOST/STOPPED from input owner/transfer state and permit status. These are proposed projection semantics for promotion, not an alternate Environment state machine. Revocation atomically fences new input/disclosure and commits audit/outbox; provider stop/queue purge happens outside SQLite and records its result. Source/geometry/incarnation change invalidates pending frames and inputs. Restart loses private handles and old cursors; reconcile durable intents before any new mutation, never restore agent input from layout state. No new migration number is assigned; T23 allocates an immutable migration after canonical promotion and collision check. Terminal profile defaults persist preference identity/version only, not credentials or authority; sessions pin actual profile/context and writer epoch.
