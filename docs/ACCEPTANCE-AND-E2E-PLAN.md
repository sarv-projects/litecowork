# LiteCowork — Acceptance and End-to-End Qualification Plan

**Status:** active evidence policy for new implementation slices.  
**Authority:** owner-domain contracts and the V1 Product Release Contract remain normative.

## 1. The qualification ladder

| Level | Meaning | What it does not prove |
|---|---|---|
| TARGET | Contract or design exists | No implementation |
| CODE | Source compiles and focused unit/component tests execute | The route is not necessarily served or reachable |
| SERVED | Authenticated API and persistence path are mounted | A real provider/agent may still be unavailable |
| INTEGRATED | Exact provider/agent/runtime and persisted path work together | The exact OS/user journey may still be unqualified |
| QUALIFIED | CODE + SYSTEM + owner USER evidence on exact supported versions, including negative/recovery behavior | Does not imply other OS/agents/providers qualify |
| BLOCKED | A required journey lacks the evidence chain | Does not mean every underlying component is absent |

Never translate a unit pass, mocked UI, schema, catalog row, successful process probe or mounted endpoint into a user-journey qualification.

## 2. Required evidence for every slice

**CODE**
- Compile/typecheck/lint the touched owner packages.
- Unit tests for valid and invalid inputs, state transitions, authorization failures, limits and idempotent replay.
- Schema/OpenAPI validation and positive/negative fixtures.
- Component tests for loading, empty, success, error, disabled, permission and stale-state projections.

**SYSTEM**
- Start the actual LiteCowork daemon/Core and the real UI transport.
- Use the real local storage and actual agent/provider adapter. Deterministic provider fixtures are acceptable only for provider output control; they must not replace the daemon, IPC, session lifecycle, persistence or UI.
- Verify actual filesystem/process/service side effects, durable rows/events/receipts and streamed projections.
- Exercise timeout-after-dispatch, duplicate request, process crash, restart, stale version, revocation, cancellation and reconciliation.
- Record exact OS/build, LiteCowork SHA, agent/provider version, protocol version, commands, exit codes and machine-readable result.

**USER**
- Owner drives the actual desktop UI with a real supported account/agent and selected Workspace.
- Capture screenshots or video for the important checkpoints, plus browser/renderer console errors and daemon logs with secrets redacted.
- Reload/restart at least once during a journey and verify the visible result against durable state.
- Record what remained blocked, unsupported or unknown. Do not turn a partial journey into a green acceptance row.

## 3. Golden end-to-end journeys

| ID | Journey | Minimum end state |
|---|---|---|
| E2E-01 | Fresh install → Workspace → select native lead → new Conversation → send/stream/stop → reload | Committed USER/AGENT messages, exact turn/session provenance, no duplicate replay |
| E2E-02 | Switch lead A → B → A | Same canonical Conversation; bounded semantic context; no private transcript or credential transfer |
| E2E-03 | Attach folder → index supported Resources → ask grounded question → revoke root | Exact revision citations; revoked root excluded from future attachment/retrieval |
| E2E-04 | Conversation → explicit Task promotion → accept Plan → admit Attempt → edit one file | Exact isolation attestation, lease, Effect receipt, independent verification and restart recovery |
| E2E-05 | Connect app → read → approve one write → reconcile → revoke | Scopes and SecretRef custody verified; no blind retries; later invocation fenced |
| E2E-06 | Discover native package → preview/adopt or install through LiteSPM → grant/use | LiteSPM-owned plan/apply receipt; Core-owned grant/effect; native config preserved |
| E2E-07 | Create Artifact → edit/version/export/reopen | Immutable versions, correct format fidelity and source provenance |
| E2E-08 | Browser/terminal Workbench → takeover/return → cancel | Real provider, explicit consent and epochs; stale handles cannot act |
| E2E-09 | Optional Coworker → chat → scoped context → revoke → restart | No cross-Coworker or cross-Workspace leakage |
| E2E-10 | Scheduled and event Automation → occurrence → Task → notification | Exactly-once occurrence identity, timezone/misfire handling, recovery and truthful delivery |
| E2E-11 | Rich UI response with presentation disabled | Semantic message remains complete; rendered controls cannot forge or settle host state |
| E2E-12 | Backup/export → restore/import → inspect provenance | Data integrity, secret exclusion, tombstone and evidence preservation |

### Optional future extension — external WorkflowBackend adapter (not a current V1 release gate)

Do not run or report this journey as available until an owner has promoted Feature 010 T05,
Feature 005 Task/Attempt admission, Feature 006 CapabilityInvocation/Connection integration,
and a provider-specific adapter. Provider-native AI builders used directly in n8n/Activepieces
are external-provider functionality and do not constitute this LiteCowork E2E.

| ID | Journey | Minimum end state |
|---|---|---|
| E2E-13 (conditional) | Connect a user-owned workflow provider → choose one explicitly MCP-exposed workflow → inspect provider/version/scopes → validate pinned Routine inputs → run one bounded provider workflow inside a LiteCowork Task → inspect/reconcile result → revoke Connection | Exact provider/instance/version and exposed workflow set; least-privilege read/build/run scope; Task/CapabilityInvocation/Effect receipt; timeout-after-dispatch and duplicate replay reconcile the same provider run; provider node states remain external observations; revoked scope blocks later calls; independent Task acceptance is not inferred from provider test/success status |

For a later direct LiteCowork-agent authoring extension (010-T06, deferred and not authorized),
add a separate journey proving read/list versus draft/mutate versus validate/test versus
publish/activate versus production-run versus delete permissions. Bind the owner approval to
the exact provider graph digest/revision, scope/effect summary and operation. Test stale
revision conflicts, destructive delete denial, MCP revocation, provider advisory/version
blocking, ambiguous publish, provider restart and audit/receipt preservation. This must not
be merged into E2E-13 invocation qualification by default.

## 4. Real UI testing rules

- A Playwright suite that mocks every backend route is a frontend component test, not full-stack E2E.
- At least one browser/desktop suite must start the actual daemon and connect through the shipped IPC/Operator API.
- Electron/Tauri or the actual packaged shell must be exercised for install, first-run, permissions, navigation, panels, native dialogs, app close/reopen and recovery.
- The UI must be driven through user interactions. Do not inspect only React state or call handlers directly.
- For consequential actions, assert both the visible approval result and the authoritative Core/SQLite/receipt outcome.
- For a disabled or unavailable control, assert a specific explanation and a non-mutating failure path.

## 5. Negative and recovery matrix

Every relevant slice must include:
- wrong Workspace, wrong principal, wrong AgentBinding, stale revision and expired grant;
- duplicate idempotency key with identical and conflicting payload;
- provider timeout before dispatch and after dispatch;
- crash between external mutation and local receipt;
- process-tree child survives parent exit;
- user cancellation during streaming or provider work;
- agent/daemon/bridge restart, session loss and incompatible version;
- symlink/path traversal, denied root, network policy mismatch and size/time limits;
- credential refresh/revocation and secret redaction;
- schema/tool drift, partial package install and native config drift;
- daylight-saving transition, missed occurrence and duplicate event delivery;
- corrupted backup, partial restore and deletion/purge retry.

UNKNOWN is unresolved, not failed or successful. A retry is permitted only after the relevant owner can prove not-dispatched or safe same-key idempotent replay.

## 6. Evidence bundle format

Each accepted journey stores:
- journey ID, requirement IDs and owner contracts;
- repository SHA and dirty/clean status;
- OS/build/architecture and packaging/runtime mode;
- agent/provider/LiteSPM versions and negotiated protocol;
- CODE commands/results, SYSTEM trace and USER evidence links;
- relevant request IDs, correlation IDs, event sequences and receipt/evidence references;
- negative cases attempted and their outcomes;
- remaining limitations and unsupported capability cells;
- reviewer and acceptance date.

Do not store credentials, access tokens, raw SecretLease bytes, private prompts or unredacted user content in evidence bundles.

## 7. Release gate

A release row moves to QUALIFIED only when the full chain is linked and reproducible on the declared target. Qualification is scoped to the exact OS/build, agent version, provider route, auth mode, protocol and feature set. Other rows remain unsupported or blocked. WSL2 is not native Windows or native Linux desktop evidence.

The historical c7e9c05 acceptance matrix in docs/archive/legacy-implementation-c7e9c05 records all ten core product journeys as BLOCKED. Re-run the inventories from the new live implementation before updating any release status.


## Desktop stop/update qualification gate - future and currently blocked

The [Runtime and Worker Lifecycle](RUNTIME-LIFECYCLE.md) contract explicitly says Task/Attempt drain, Effect reconciliation, lease fencing/release and local service-reference settlement are not yet integrated into authenticated stop/stop-preview. Therefore a desktop quit/update control must not be described as a domain-complete stop or safe update today.

When the owner domains define and implement that protocol, its acceptance journey must use the actual packaged Operator and Runtime and include at least: idle stop; active admitted Task/Attempt; a dispatched but unresolved external Effect; outstanding ExecutionLease and EnvironmentControlLease; capability/provider references; child process that outlives its parent; pending cleanup; simultaneous stop/update requests; admission racing with drain; crash/timeout; and restart/recovery. Assert that new admission is fenced, admitted work is reconciled according to its owner contract, external side effects remain UNKNOWN until proven, leases/references are fenced or released by their owners, and update/install does not proceed without positive quiescence evidence. The UI must explain what is still running or unknown and offer a safe retry path.

DeepSeek Harness's admission-lock/drain/reinspect/graceful-ack sequence is a research reference in [DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md](DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md), not a LiteCowork acceptance pass. This gate does not imply that a stop/update API, handler or implementation exists.
