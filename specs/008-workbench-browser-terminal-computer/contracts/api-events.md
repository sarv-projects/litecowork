# API and events - Workbench Browser Terminal and Computer

> **Revision 1.1: IN_REVIEW / TARGET (2026-11-12).** Actual laptop Computer view and independent local/agent observation/input replace the requested browser-only scope. Historical revision 1.0 acceptance remains below; implementation remains UNAVAILABLE. See [Computer LLD](../../../docs/COMPUTER-OBSERVATION-AND-CONTROL-LLD.md), SPEC section 20 and tasks T22..T27.

**Historical baseline status:** Revision 1.0 was ACCEPTED/LOCKED on 2026-11-09; revision 1.1 remains IN_REVIEW/TARGET and authorizes no route or event promotion until its owner gates close.

## Current served API surface

The composed daemon route inventory contains no BrowserSession create/list/get/close, browser tab/navigation/capture/action, BrowserControlLease/takeover/return, terminal profile discovery, TerminalSession create/input/resize/stop/reconnect, or Computer observation/input operation. `takeEnvironmentControl` and `returnEnvironmentControl` are OpenAPI target-only Task/Attempt Environment operations; they are not browser-control handlers and no current Workbench call site invokes them.

The generated OpenAPI TypeScript definitions for target Environment control operations do not establish a served handler. `docs/schemas/served-route-inventory.json` classifies the take/return control routes and Environment provision/lifecycle routes as target-only.

## Compiled policy code is not an API

`browser_control_service.rs` exposes pure helper logic for actor identity, epoch/session/tab revisions, lease owner, in-flight input, takeover and return. It is compiled with `allow(dead_code)` and has no composed Operator route, durable production lease store or provider dispatch consumer. `browser_consent.rs` contains profile/consent validation only.

`environment_computer_use.rs` exposes policy types but no provider/route. It explicitly reports no qualified screen-capture/input-injection backend; every request is refused. It is compiled with `allow(dead_code)`.

No PTY/ConPTY Operator API, Tauri spawn/input/resize bridge or daemon-owned TerminalSession record was found. The Workbench UI's React-local command handler is not an API.

## Target route/schema promotion

Before adding browser session/control, terminal session or computer actions, owners must promote the operationIds, method/path, request/response schemas, events and errors in the canonical `docs/API.md`, `docs/EVENTS.md`, `docs/SCHEMAS.md` and `docs/schemas/` documents in the same reviewed change. Do not invent operationIds or use generated API type presence as proof a handler exists.

## Target Browser ordering

Authenticate the principal; validate Workspace/Task/Attempt/Environment scope and consent; compare the exact session revision, tab generation, control epoch and lease owner; reject expired/stale/in-flight takeover; commit idempotent intent/lease receipt; invoke the browser provider outside SQLite; persist observation/acknowledgement and reconcile ambiguous actions. A takeover fences queued agent input; return requires fresh provider observation and a new epoch. Never replay stale selectors or forms after restart.

## Target Terminal ordering

Discover shell profiles without executing shell startup scripts; bind the chosen profile and exact executable/argv/cwd/environment allowlist to an admitted Environment; commit session owner/receipt before spawning the OS PTY/ConPTY outside SQLite; attach the process to the exact containment scope; stream bounded output with backpressure; persist process/session identity and owner; prove quiescence or lost state before release/recovery. A UI view attach is not a spawn.

## Target Computer-use ordering

Require explicit OS permission and Workspace-scoped consent; verify excluded sensitive regions and per-action capture/injection scope; admit only a qualified provider and declared action type; reconcile capture/input outcomes; revoke/expire consent and fence in-flight actions safely. Under the current revision, Computer remains unavailable until revision 1.1 scope and machine contracts are owner-approved and its exact OS/provider is qualified. B-32's accepted revision 1.0 deferral remains historical, not a served Computer API.

## Events and error boundary

Use the exact event contracts owned by `docs/EVENTS.md` and `docs/schemas/domain-event.schema.json`, including `environment.control.lease.changed.v1` only when its owner operation is served. BrowserActionReceipt, TerminalSessionReceipt and EnvironmentControlLease remain target records until promoted. Reuse canonical error codes; do not invent local error strings as API contract.

## Accepted target interfaces (not yet in source, 2026-11-09)

BrowserSession, BrowserControlLease (take/return), BrowserActionReceipt, TerminalSession (create/input/resize/stop/attach), CommandReceipt, Workbench Activity read and RevisionDiff read are accepted target interfaces. Their operationIds, payloads, events and error codes are promoted into CHROME-BROWSER-LLD, V1-INTEGRATED-TERMINAL-LLD, `docs/schemas/operator-api.openapi.yaml`, `docs/EVENTS.md`, `docs/schemas/domain-event.schema.json` and `docs/schemas/error-codes.schema.json` by 008-T01 in one change. Until then none is served and no name here is a route.


## Revision 1.1 promotion map

Use the Computer LLD target-port table as design input to T23: inspect capabilities, choose/start source, subscribe/detach frames, set/revoke agent observation permit, take/return input, dispatch/reconcile action, stop/purge and explicitly save snapshot. Do not treat these labels as routes. T23 assigns exact API/OpenAPI operations, requests, responses, ACLs, event names/version and errors in their canonical owners together. Local IPC request_id remains correlation; Idempotency-Key provides deduplication, If-Match provides CAS and X-Workspace-ID scopes the request. Principal never comes from model/UI fields.

Input binds exact physical desktop lease/fence, RuntimeIncarnation, target/capture/geometry generations, source observation sequence, agent Task/Attempt and bounded action digest; binary frame streams are separately authenticated ephemeral subscriptions with independent audience permits. Observation is not blocked merely because USER owns input. A frame does not become a durable event. Proposed session/permit/control/action/stop event payloads contain only IDs/revisions/reasons/policy digest/assurance, never pixels, raw key text, private handles or full application/URL metadata. Existing EnvironmentControlLease remains the authority; schema promotion must not allocate a duplicate computer lease. Terminal adds validated profile defaults and single stdin writer epoch/input sequence to existing session owner contracts.
