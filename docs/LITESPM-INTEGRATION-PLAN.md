# LiteCowork ↔ LiteSPM — Integrated Service Plan

**Status:** Revision 9 target architecture and delivery plan; not a claim of current qualification.
**Owners:** LiteSPM owns package/catalog/inventory/config-mutation and package-runtime lifecycle. LiteCowork owns Workspace/Coworker/Task authorization, grants, invocation, Effects, Evidence and the user-facing service supervisor.
**Current evidence boundary:** the supplied Revision 9 pins LiteSPM at 7127c8d3 and LiteCowork at c7e9c05. The existing LiteCowork interoperability LLD records a candidate local CLI/MCP route and an unqualified integration. Re-probe current main and exact shipped protocol before implementation.

## 1. Target architecture

LiteSPM is a **first-class integrated local service**, not merely an optional CLI button and not a second database embedded in LiteCowork.

- One LiteSPM daemon, package database, catalog/cache and content-addressed store per OS user.
- A thin, separately registered bridge for each compatible native agent/host.
- LiteCowork installs or updates the signed LiteSPM service, supervises daemon lifecycle, performs attach/health/recovery and exposes the native Marketplace UI.
- LiteSPM remains authoritative for package inventory, package resolution, install plans, owned mutations, deployment ledger, package process lifecycle and operation receipts.
- LiteCowork remains authoritative for who may use a capability in a Workspace/Conversation/Task, which agent may receive it, the scoped grant, invocation admission and any mediated Effect/Evidence.

Do not launch a private LiteSPM daemon for each agent. Do not duplicate its database, resolver, package IDs, catalog or CAS in LiteCowork. Do not mutate LiteSPM internal storage directly.

## 2. Automatic agent-to-service reconciliation

On supported agent install, discovery, update, reinstall, remove and first use, LiteCowork runs Agent Capability Service Reconciliation (ACSR).

The sequence is:
1. Observe the exact agent, version, OS, native config format and existing LiteSPM bridge registration.
2. Check compatibility and freshness using the qualified AgentModule/LiteSPM host adapter.
3. Apply the user's one-time standing auto-connect policy if it exists and the host strategy is qualified.
4. Otherwise present an explicit Connect/Repair action or the precise MANUAL_SETUP_REQUIRED state.
5. Register the agent bridge once, idempotently, against the shared LiteSPM daemon.
6. Perform the authenticated handshake and store the observed protocol/health state.
7. Project agent installed/authenticated state separately from LiteSPM connected/degraded/unsupported state.

A user who declined automatic connection gets NOT_CONNECTED_BY_POLICY and no config write. Unknown host config syntax is manual-only. No silent guessing, blind merge or replacement of sibling host entries. One-time consent governs only qualified connection of the bridge; it does not grant package installation or operation use.

## 3. Process and trust boundaries

**LiteCowork ServiceSupervisor**
- Installs only a signed/digest-verified LiteSPM binary.
- Owns start/attach/readiness/health/restart/update/rollback of the service process.
- Handles cold startup and daemon reattach without creating a second instance.
- Performs recovery before exposing service-ready UI.
- Provides an exact uninstall/restore flow; service removal and data deletion are different explicit actions.

**LiteSPM daemon**
- Owns its SQLite/package database, catalog/cache, CAS, host registration, deployment mutation, receipts, package provider process and package lifecycle.
- Publishes an authenticated local IPC descriptor and versioned schemas.
- Keeps its standalone CLI and public marketplace fully usable without LiteCowork.

**Per-agent bridge**
- Uses the host's documented config location and supported mutation strategy.
- Contains only the required bridge registration/launch parameters.
- Does not receive LiteCowork secret bytes or become the authority for Core grants.

**LiteCowork Core**
- Issues an approval only for the exact LiteSPM plan digest, target, host, scope and expiry.
- Obtains/handles the LiteSPM single-use approval token through the qualified protocol; the agent cannot approve itself.
- Records the Core-side correlation/receipt and separately projects package state and invocation readiness.
- Invalidates relevant grants when package tool schema or permission fingerprint changes.

## 4. IPC/client requirements

Implement a typed LiteSpmClient against the exact published LiteSPM protocol release. The service descriptor must identify protocol version, daemon identity, supported operations, endpoint, authentication challenge, limits and health.

Prefer OS-protected local IPC (named pipe on Windows, Unix domain socket on Linux/macOS) with peer authentication and version negotiation. Reject remote network binding for the local control plane in V1.

The client must:
- Verify daemon identity, protocol compatibility and peer identity.
- Enforce maximum request/response sizes, timeouts and bounded pagination.
- Preserve original LiteSPM error code and operation ID in diagnostics.
- Use only operations advertised by that exact version.
- Never construct arbitrary shell commands from package/user/model input.
- Never accept a successful empty inventory when the daemon actually failed.
- Never expose tokens, approval values, environment secrets or raw untrusted skill contents in logs.

The LiteSPM service's actual wire methods and schemas are published by LiteSPM. This plan describes semantics, not guessed method names. Do not scrape CLI output or invent RPC names. Keep the existing 12 MCP bridge tools and compatible /marketplace affordance; extension is preferable to forking the agent-facing ABI.

## 5. Marketplace and package lifecycle

The Marketplace consumes the same canonical LiteSPM catalog IDs, sources, versions, provenance and installability state as the public LiteSPM site.

The UI must distinguish:
- Explore/catalog listing;
- Already on this computer / observed native registrations;
- Managed LiteSPM installations;
- Adopted native registrations;
- Agent bridge connection;
- provider/account Connection;
- Coworker/Workspace assignment;
- active scoped Grant;
- Activation and actual Invocation.

A package catalog entry is not necessarily installable. Installed does not imply connected, granted, activated, invocable or verified.

Lifecycle verbs are distinct:
- **Use existing:** select a qualified pre-existing registration without changing it.
- **Manage with LiteSPM / Adopt:** explicit ownership transfer after observation and preview.
- **Install to agent:** install an exact package/version to a specific host/scope.
- **Copy to another agent:** distinct install/registration and host qualification.
- **Update / Repair:** exact plan and current fingerprint required.
- **Stop managing / Unmanage:** relinquish lifecycle ownership but preserve pre-existing native config.
- **Remove installation:** remove only exact unchanged LiteSPM-owned nodes/files under an approved plan.
- **Disconnect bridge:** remove the LiteSPM host registration only.
- **Uninstall service:** remove the local service through its own plan.
- **Delete local data:** separate irreversible action that enumerates exact data roots and requires export/backup warning.

No whole-file restore over user edits. Drift is read-only until the user chooses a reviewed resolution. Unknown external edits remain preserved by default.

## 6. Approval and crash recovery

1. Observe current host/package state and its exact fingerprint.
2. Prepare an immutable plan with exact targets, scope, version, pre-image, intended diff, hazards and expiry.
3. Display the plan in LiteCowork.
4. User approves the exact plan hash.
5. Core obtains a single-use approval token bound to that plan and owner identity.
6. LiteSPM revalidates the plan/fingerprint and applies with stable idempotency.
7. LiteSPM persists its operation receipt and performs read-back.
8. LiteCowork refreshes inventory and projects observed/managed/adopted state.
9. A lost response is queried by original operation identity. UNKNOWN must reconcile before another mutation is attempted.

Filesystem mutation and SQLite receipt are not one atomic medium. LiteSPM must preserve intent/journal, exact pre-image and recovery/read-back so it can resolve crashes between external write and local commit. Core must not report success from a request timeout.

## 7. Ordered delivery sequence

| Step | LiteSPM work | LiteCowork work | Exit evidence |
|---:|---|---|---|
| 0 | Re-read current HEAD, STATUS, ARCH, schemas, methods, tests and releases | Re-read current routes, migrations and acceptance matrix | Exact baseline and gap matrix |
| 1 | Freeze/publish IPC handshake and generated client contract | Typed LiteSpmClient, version negotiation, peer auth | Real handshake and compatibility fixtures |
| 2 | Daemon cold start/attach, autostart, stale-lock recovery, health | ServiceSupervisor, signed install/update/rollback | Cold machine start, one daemon, crash recovery |
| 3 | Harden host prepare/apply/verify/remove/reconcile and receipts | ACSR plus one-time auto-connect consent | Agent install/discovery → bridge handshake; no config clobber |
| 4 | Machine-readable doctor and host/provider health | Registry status and repair/manual fallback UI | Correct failure isolation and restart behavior |
| 5 | Catalog sync, release integrity, search/detail IPC | Native Marketplace using same canonical IDs | Catalog parity and stale/offline honesty |
| 6 | Plan/apply approval correlation and package lifecycle | Install/update/remove UX and Core approval | Exact plan → approval → apply → receipt |
| 7 | Verify package kinds and preserve bridge ABI | Existing 12 MCP tools and /marketplace experience | Agent can request but cannot self-approve |
| 8 | Capability discovery, provider health and schema drift | Agent inventory, tool details, scoped grants | Installed/catalog/invocable clearly distinct |
| 9 | Lockfile/export/import/copy | Cross-agent portability UX | Reproducible package state; no credential/session copy |
| 10 | Provenance, advisories, signatures and compatibility | Trust and source evidence UI | No unsupported trust badges |
| 11 | Host × OS × version × protocol conformance | Full user qualification | Positive, negative, drift and recovery evidence |

This sequence may run alongside independent native Conversation work. It does not make unqualified package operations available.

## 8. Acceptance matrix (LSI-001–LSI-040)

| ID | Scenario | Expected behavior |
|---|---|---|
| LSI-001 | Fresh LiteCowork install, no LiteSPM | Signed service binary installed once; signature, digest and version verified |
| LSI-002 | LiteCowork startup | One daemon starts; readiness handshake and recovery complete before service-ready |
| LSI-003 | Two agents discovered simultaneously | One daemon; two idempotent host registration operations |
| LSI-004 | Agent installed while daemon stopped | Supervisor starts daemon; registration completes or remains recoverable |
| LSI-005 | Auto-connect consent enabled | Qualified bridge applied and recorded |
| LSI-006 | User declined auto-connect | No config write; NOT_CONNECTED_BY_POLICY; explicit setup available |
| LSI-007 | Unknown host config format | Manual setup only; no guessed path or mutation |
| LSI-008 | Existing sibling entries/comments | Syntax-preserving merge; siblings unchanged |
| LSI-009 | Repeated detect/restart | No duplicate entry, registration or ledger row |
| LSI-010 | Crash during host apply | Recovery determines outcome; no false connected state or clobber |
| LSI-011 | Windows stale lock after forced crash | Safe recovery without manual lock-file deletion |
| LSI-012 | LiteSPM below minimum version | Update required; unsupported RPC calls refused |
| LSI-013 | Invalid service signature | Activation refused; prior known-good version retained |
| LSI-014 | Bridge handshake fails | Agent remains installed; LiteSPM degraded; repair shown |
| LSI-015 | Agent config externally edits bridge | Drift detected; repair preview preserves user edits |
| LSI-016 | Agent uninstalled | Only its bridge entry removed; package data retained absent separate approval |
| LSI-017 | Catalog search | Canonical IDs, current release, filters and pagination work |
| LSI-018 | Discovery-only package | Install disabled with explicit reason |
| LSI-019 | Unsupported plugin components | Partial install refused unless explicit acknowledgement is bound to plan |
| LSI-020 | Agent prepares install | Plan created; no filesystem mutation |
| LSI-021 | Agent requests install without approval | Refused; no mutation |
| LSI-022 | User approves in LiteCowork | Exact planHash-bound single-use token; operation executes once |
| LSI-023 | Approval token replay | Refused; no second mutation |
| LSI-024 | Approval after plan changed/expired | Refused; re-prepare required |
| LSI-025 | Package install completes | LiteSPM receipt and Core projection reflect exact target/scope |
| LSI-026 | User modifies installed host config | Three-way reconcile detects drift and preserves edits |
| LSI-027 | Update changes permission/schema | Diff shown; grants invalidated; re-review required |
| LSI-028 | Package removal | Only unchanged LiteSPM-owned nodes/files removed |
| LSI-029 | Catalog offline | Cached listing shown stale; unavailable artifact cannot install |
| LSI-030 | Catalog digest mismatch | Update refused; last verified release retained |
| LSI-031 | Provider process unhealthy | Degraded status; no false healthy/invocable state |
| LSI-032 | MCP schema drift | Invocation refused pending grant/schema review |
| LSI-033 | Two bridges invoke concurrently | Shared daemon serializes mutations and bounds invocation concurrency |
| LSI-034 | LiteCowork restarts while LiteSPM runs | Reattach to same daemon; no split brain |
| LSI-035 | LiteSPM restarts while bridge exists | Bridge reconnects without duplicate daemon/registration |
| LSI-036 | Uninstall LiteSPM service | Service/bridge removal is planned; package ownership/data deletion separate |
| LSI-037 | Delete LiteSPM data | Export/backup warning, irreversible confirmation and exact paths |
| LSI-038 | Public site and desktop Marketplace | Same canonical IDs, release and installability |
| LSI-039 | Accessible/no-JS package detail | Identity, source, installability and docs remain readable |
| LSI-040 | Per-host conformance | Exact OS/version/config, handshake, repair, remove and recovery evidence |

## 9. Release gate and status truth

Revision 9 says integrated LiteSPM is not qualified until the required P0 rows and LSI-001–016, LSI-020–030 and LSI-034–038 pass on declared target OS(s), with per-version evidence for supported hosts. Remaining provider health/invocation gaps must stay explicitly scoped and disabled where required.

LiteSPM evidence labels such as DESIGNED, IMPLEMENTED, WIRED, TESTED, VERIFIED and SHIPPED belong to LiteSPM's own repository. LiteCowork uses CODE, SYSTEM and USER qualification. WIRED is not QUALIFIED; a published marketplace page is not an installed/connected package manager.

The existing docs/LITESPM-LITECOWORK-INTEROP-LLD.md remains the detailed owner-facing interop contract and must be updated consistently when the upstream protocol is pinned. This plan is a delivery map, not a declaration that ACSR or the shared service is already implemented.
