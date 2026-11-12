# LiteCowork ↔ LiteSPM — Integrated Service Plan

**Status:** SUPERSEDED research proposal. Feature 013 remains owner-deferred. This plan's daemon-supervisor/service-descriptor approach is not the current target and is not authorized for implementation.
**Owners:** LiteSPM owns package/catalog/inventory/config-mutation and package-runtime lifecycle. LiteCowork owns Workspace/Coworker/Task authorization, grants, invocation, Effects, Evidence and the user-facing service supervisor.
**Evidence correction (2026-11-12):** this document is a target proposal, not an accepted LiteCowork contract. The upstream checkout at `84d5faba75bc621a651e0cec0f831e936bb2082d` (`v0.6.0-143-g84d5fab`) was read read-only on 2026-11-09, but is not an owner-approved integration pin. The docs disagree on the MCP bridge tool count (12 in the 84d5fab study, 13 at the older 111fd9b8 snapshot), so do not freeze a count or map daemon methods to tool names. Root ARCHITECTURE.md identifies the published local MCP stdio bridge as the integration boundary; direct daemon IPC and the service-descriptor design below are superseded. Feature 013 remains DEFERRED and its Runtime/process ownership, exact published tool schemas, owner approval and recovery semantics remain unresolved. See [LITESPM-INTEGRATION-STUDY-2026-11.md](LITESPM-INTEGRATION-STUDY-2026-11.md) and [Feature 013](../specs/013-litespm-package-lifecycle-bridge/README.md).

> **Current boundary:** root ARCHITECTURE.md and AGENTS.md require LiteCowork to use LiteSPM's exact published, pinned local MCP stdio bridge through a Runtime-owned process/session boundary. No LiteSPM daemon IPC, private service descriptor or bridge tool count is approved by this document. Plan approval, tool schemas, process identity and UNKNOWN recovery must be proved at the selected pin before any verb is enabled. Sections below preserve an earlier design exploration only.

## 1. Superseded service proposal (historical; do not implement)

Everything in this plan is TARGET only and remains subject to Product, Architecture, Runtime and LiteSPM owner review. Feature 013 is DEFERRED; this plan does not authorize service installation, package mutation, process spawning or a new LiteCowork transport. If the published bridge cannot preserve LiteCowork approval and recovery guarantees, the supported product scope may be read-only or unavailable.

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
- Exposes a published local MCP stdio bridge. Do not assume it publishes a separate LiteCowork daemon descriptor or authenticated direct-RPC endpoint.
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

## 4. MCP client and process-identity requirements

Use only LiteSPM's exact published, pinned MCP stdio bridge contract as the integration boundary. Do not add direct daemon IPC, named-pipe/socket methods, a private service descriptor or an invented authentication challenge. At the owner-selected pin, validate MCP protocol negotiation, server information, bounded tool manifest and each input/output schema. The current documents conflict on whether the bridge exposes 12 or 13 tools; the count and names must be re-read at the selected pin.

The LiteCowork client consumes a session supplied by the owner of local Runtime/process lifecycle. That owner defines how the stdio process is launched, supervised, stopped and identified on each OS. Stdio and MCP metadata alone do not prove process identity. If exact process identity evidence or an operation's schema is unavailable, keep the integration or that operation unavailable. Do not route around a missing tool through private daemon RPC.

The client must:
- Enforce maximum request/response sizes, deadlines and bounded pagination.
- Preserve published LiteSPM error codes and operation identifiers only when the MCP result actually carries them.
- Use only tools present in the pinned manifest and qualified for the exact host/OS/scope/operation profile.
- Preserve the owner-approval digest, stable idempotency and UNKNOWN reconciliation contract; if the published tools cannot support it, keep mutation disabled.
- Never construct arbitrary shell commands from package/user/model input.
- Never accept a successful empty inventory when the MCP call actually failed.
- Never expose tokens, approval values, environment secrets or raw untrusted skill contents in logs.

Do not scrape CLI output or infer RPC/tool names. Host UX is version-specific: OpenCode v2.2.5 exposes `/litespm:marketplace`; Qwen Code 0.14.4 exposes tools under `/mcp` rather than a slash command. These are upstream host examples, not LiteCowork UI requirements. Do not assume universal `/marketplace` parity or add an incompatible second ABI.

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
| 1 | Pin the published MCP protocol/tool schemas and process/session owner | Typed LiteSpmClient, bounded manifest validation, exact process-identity evidence | MCP conformance fixtures and owner-reviewed Runtime boundary |
| 2 | Decide whether LiteCowork owns service install/supervision or only attaches to an existing LiteSPM service | ServiceSupervisor only after Architecture/Runtime/Product approval | Owner decision, cold-start and crash-recovery evidence for each OS |
| 3 | Harden host prepare/apply/verify/remove/reconcile and receipts | ACSR plus one-time auto-connect consent | Agent install/discovery → bridge handshake; no config clobber |
| 4 | Machine-readable doctor and host/provider health | Registry status and repair/manual fallback UI | Correct failure isolation and restart behavior |
| 5 | Catalog sync, release integrity, search/detail IPC | Native Marketplace using same canonical IDs | Catalog parity and stale/offline honesty |
| 6 | Plan/apply approval correlation and package lifecycle | Install/update/remove UX and Core approval | Exact plan → approval → apply → receipt |
| 7 | Verify package kinds and preserve bridge ABI | Current 13-tool stdio bridge; host-specific prompt/tool entry points | Agent can request but cannot self-approve |
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
