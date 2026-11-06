# API and events - Workbench Browser Terminal and Computer

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

Require explicit OS permission and Workspace-scoped consent; verify excluded sensitive regions and per-action capture/injection scope; admit only a qualified provider and declared action type; reconcile capture/input outcomes; revoke/expire consent and fence in-flight actions safely. If B-32 defers V1 Computer use, no Computer-use operation is served.

## Events and error boundary

Use the exact event contracts owned by `docs/EVENTS.md` and `docs/schemas/domain-event.schema.json`, including `environment.control.lease.changed.v1` only when its owner operation is served. BrowserActionReceipt, TerminalSessionReceipt and EnvironmentControlLease remain target records until promoted. Reuse canonical error codes; do not invent local error strings as API contract.
