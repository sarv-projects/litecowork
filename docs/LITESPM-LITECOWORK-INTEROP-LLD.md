# LiteSPM ↔ LiteCowork — Local Package Management and Invocation Interoperability LLD

**Status:** binding integration **TARGET**, not a published compatible wire protocol or a qualified connection.  
**Owners:** LiteSPM owns package/catalog/inventory/config-mutation authority; LiteCowork owns Workspace/Coworker/Task authorization, execution and evidence.  
**Prerequisites:** LiteSPM must publish versioned machine endpoint and operation schemas. Do not build client calls by scraping CLI output or guessing the RPC name.  
**References:** [Product](V1-PRODUCT-RELEASE-CONTRACT.md), [Capability Fabric](CAPABILITY-FABRIC.md), [Capability Invocations](CAPABILITY-INVOCATIONS.md), [Agent Control](AGENT-CONTROL.md), [Trust](TRUST.md), [LiteSPM ARCH/33](https://github.com/sarv-projects/LiteSPM/blob/main/ARCH/33-DEPLOYMENT-LEDGER-RECONCILIATION.md), [ARCH/38](https://github.com/sarv-projects/LiteSPM/blob/main/ARCH/38-CLI-PRODUCT-SURFACE.md).

## 1. Problem and ownership

Without LiteSPM, a user repeats package setup across Codex, Claude Code, OpenCode, HorizonCode and other compatible agents. LiteSPM shall manage both eligible **existing native capabilities** and new installs without forcing reinstallation or flattening incompatible host semantics. Its inventory is the source of **observed package state**, not a substitute for agent-native runtime discovery.

LiteCowork must not duplicate LiteSPM's install database, writer, package resolver or version lifecycle. It may store stable external package refs, the approved operation receipt, and its **separate** CapabilityAssignments, ConnectionRefs, Grants, Activations and Invocations. One installed package is not proof of Coworker access or Task-time tool authority. LiteCowork does not assume all native actions go through its Gateway.

## 2. Concepts with distinct identity

| Concept | Responsible owner | Explanation |
|---|---|---|
| Listing / Source | LiteSPM | A catalog object and source provenance; may not be downloadable/installable |
| Native Observation | LiteSPM host adapter | Existing config/skill/plugin/host capability read with timestamp/fingerprint; **read-only**, no ownership |
| InstallRecord / Component / DeploymentMutation | LiteSPM | Exact owned or adopted locator + previous/current image; lifecycle and reconciliation authority |
| HostRegistration | LiteSPM | A capability configured for one agent, scope and location; not universal package-equivalence proof |
| ManagementOperation | LiteSPM | Idempotent plan/apply/update/repair/unmanage/remove operation; durable receipt and UNKNOWN recovery |
| AgentBinding + AgentCapabilityBridge | LiteCowork | The qualified native harness and route by which capabilities may be attached or exposed |
| Connection / SecretRef | Credential-owning service | Account credentials and provider access; opaque handle only outside vault |
| CoworkerCapabilityAssignment | LiteCowork | Owner-controlled allowed subset; not a live action grant |
| CapabilityGrant / Invocation / Effect | LiteCowork | Actual execution admission, receipt, reconciliation and verification where mediated |
| Native external action | Native harness/provider | Only observed/reported unless independently qualified for enforcement |

An external agent may have manually registered GitHub MCP in several different native profiles. Preserve individual host registrations and do not dedupe by display name; identity resolution requires canonical source, versions, config identity and evidence.

## 3. Inventory and lifecycle state

Required inventory projection per item: `package_ref?`, `host_id`, `host_version?`, `scope(user|project)`, `locator`, `source/provenance`, `native_origin`, `management_state`, `observed_fingerprint`, `last_observed_at`, `compatibility_evidence`, `readiness`, `repair_reason?`. Omit unknown optional values; never invent package identity from a human name.

State names **managed**, **observed**, **adopted**, **drifted**, **broken** describe lifecycle/inspection, not tool readiness. `Needs auth`, `stopped`, `unqualified`, `unsupported` and `unknown` require distinct runtime/readiness fields. When read fails, return an explicit unavailable error, not an empty successful list.

| Origin / state | Safe operations |
|---|---|
| Observed native capability | Read/inventory, describe, compatibility check, supported test; offer explicit **Adopt** with preview |
| Managed LiteSPM-created install | Qualified update/reconcile/repair/remove/attach; never remove unowned siblings |
| Adopted external locator | Update/reconcile under recorded ownership; **Unmanage** leaves native entry; deleting originally present entry by default is forbidden |
| Drifted | Nonmutating structural diff; owner chooses keep-local, use-package, detach or cancel; default keep-local |
| Broken / missing | Explain cause; do not silently recreate, steal native config or claim successful repair |
| Unknown/freshness expired | Reject mutation or demand new observation; do not use stale approval on a changed fingerprint |

## 4. Native host/format qualification

Adapter support is **per host × version × OS × scope × capability kind × operation**. MCP config-reading support does *not* imply skill lifecycle, plugin lifecycle, native hook ownership, command registration, OAuth or runtime execution support. Generic config writers require read-back conformance tests with actual native host versions. Hosted account-managed extensions and volatile in-memory native sessions may be impossible to inventory or adopt.

When the host owns a private extension configuration, LiteSPM can offer observation or native-settings handoff, not blind rewrites. For capabilities already managed by a native package manager, prefer cooperative ownership or observe-only; `adopt` must not break that manager's future updates.

## 5. Machine interface to agree and qualify

Define a versioned **authenticated local** LiteSPMServiceDescriptor (protocol version, identity, endpoint, supported operations, maximum input/response sizes, challenge/auth mechanism, health/availability). LiteCowork only uses operations advertised and qualified by that exact version. No stringly typed shell command constructed from arbitrary package input.

**Semantic operations (not assumed wire method names):**

| Operation | Request contract | Result / refusal |
|---|---|---|
| inventory | host set, scopes, cursor, observation freshness | items, provenance, observations, pagination, stale/unavailable |
| describe | stable package/registration ref | kind/source/versions, compatibility, permissions, hazards and evidence |
| plan | operation, targets, expected versions/fingerprints, scope, intended conflicts | immutable plan digest, exact diff, risk, grants not included, expiry |
| apply | plan digest, expected head, owner-approved command identity, idempotency key | durable operation ID, state, mutation/receipt refs, UNKNOWN recovery |
| adopt | observed native locator, exact scope and image, explicit owner consent | ownership record; unchanged host config where supported; read-back |
| reconcile | recorded owned state + current observation | three-way change report; read-only until separate approved resolution |
| unmanage | managed/adopted registration, exact locator/version | stop lifecycle ownership, preserve originally present native entry |
| remove | owned install, current fingerprints and plan | remove owned structures only; no destructive full-file restore |
| health/test | exact registration and permitted probe | observed readiness and provenance, not an assumption from install |
| subscribe/operation-get | cursor or operation identity | bounded progress, durable result or UNKNOWN with resumption |
| invoke | separately granted capability ref, scope and effect policy | invocation result + provider/native assurance info; not a package-install authorization |

LiteSPM must own the final published wire method names, schemas, error vocabulary and version negotiation. This document must be reconciled with that upstream publication before a real client is marked integrated. Version-incompatible clients fail closed.

## 6. Plan/apply transaction and crash recovery

1. LiteCowork authenticates to local LiteSPM and obtains current service descriptor; user selects the exact package/host/scope.
2. Read observed/native state and host compatibility. Present source, origin and exact changes.
3. LiteSPM produces **read-only** mutation plan: target locator, pre-image hash, new image (or ownership record only), conflicts, replacement/rollback and requirement for user consent.
4. User approves a particular plan digest/expiry and host/scope. Approval to **manage package config** is separate from LiteCowork execution grants.
5. Apply with a stable idempotency key; LiteSPM revalidates freshness and preconditions, records recoverable operation intent, executes each owned mutation, commits durable operation receipt and read-back observation.
6. If a response is lost, query the original operation identity; if external state is UNKNOWN, inspect/journal/reconcile before retry; never launch the same unmanaged mutation blindly.
7. LiteCowork refreshes package inventory and separately offers approved Workspace/Coworker assignment and runtime use.

**Crash-window invariant:** file mutation and SQLite bookkeeping are not one atomic medium. Intent/journal + structural readback + compare are required to recover interruptions between filesystem write and DB commit. The upstream ARCH/33 explicitly identifies this window: do not mark it resolved without fault injection.

**Three-way reconciliation:** A = pre-owned locator image, B = LiteSPM-written/registered image, C = current host locator. If C differs from the owned B due to an external edit, keep-local by default. Adoption distinguishes originally existing content so that unmanage leaves it present. Never restore a whole-file backup over unrelated sibling edits.

## 7. LiteCowork authorization and effect boundary

Assigning a capability to a Coworker is an *allowlist ceiling*, not execution authorization. Admission for an actual Conversation/Task operation rechecks Workspace/Coworker assignment, exact AgentBinding, provider readiness, user policy, current grants, TTL/fences, Environment, active service and idempotency. External credentials remain opaque SecretRefs and travel only through the qualified broker/provider destination, never model-visible variables or logs.

AgentCapabilityBridge classifies the actual route:
- `DIRECT_NATIVE_ATTACHMENT`: agent executes internally. Mark reported/observed assurance unless the host-provided control hook is independently qualified.
- `LITECOWORK_GATEWAY`: mediated invocation eligible for pre-dispatch Trust decision and Core Effect/Evidence receipts.
- `HOST_CONTEXT_ONLY`: visible context/instructions with no executable capability implication.
- `UNSUPPORTED`: unavailable, exact reason and repair/alternative.

A native tool being used outside the LiteCowork Gateway does not become a LiteCowork-mediated Effect by virtue of LiteSPM adoption. Do not claim the absence of hidden native execution unless enforcement is proved at the harness/OS boundary.

## 8. User-facing interaction contracts

Discover has **Explore** (catalog), **Already on this computer** (observed/managed/adopted), **Installed** (managed per host), and **Connections** (provider account auth) as conceptually distinct views, whether implemented as tabs or filters. The first run scans only documented authorized configs and clearly labels scope and unknown availability.

Actions: **Use existing**, **Manage with LiteSPM** (adopt), **Install to agent**, **Copy to another agent**, **Update**, **Repair**, **Stop managing** and **Remove installation** are different verbs with different previews. Never make a native observed entry removable by default. Show no irreversible action before exact target and diff confirmation.

Display package availability, Connection authenticated status, Coworker assignment and current use permission as distinct facts; one green “Connected” pill cannot replace all four. If the local bridge is missing, offer supported CLI/native setup rather than fabricate a successful install.

## 9. Required negative/acceptance cases

**Inventory:** existing manually installed MCP detected without writes; unsupported host is observed-only; offline scan returns unavailable not empty; duplicate names across scopes remain distinct; stale observation fails mutation.  
**Adoption:** user/project scope separated; config byte-identical on adoption; unmanage leaves original; edited native entry remains unchanged; missing locator refused; repeated approval/idempotency leaves one row.  
**Copy/install:** plan before write; user conflict defaults keep-local; checksum/signature provenance; no arbitrary shell interpolation; each agent's native config still loads; rollback preserves sibling comments.  
**Runtime:** installed ≠ granted; revoked/expired grant cannot invoke; source-native actions labeled observed-only; provider auth expiry and process restart; failed remote action may remain UNKNOWN.  
**Recovery:** kill after file write, before DB commit, before UI acknowledgement, during config update and during unmanage; reconcile without deleting unrelated config.  
**Release:** CODE + SYSTEM + actual host USER qualification for every advertised agent/platform/kind/operation combination.

## 10. Implementation placement / sequencing

**LiteSPM:** extend existing `cmd/litespm/adopt.go`, `list.go`, `copy.go`, `internal/host`, `internal/deployment` and IPC methods. Do not invent a new independent package database. Resolve upstream `connector` kind/lifecycle decision (ARCH/26 vs ARCH/29) before claiming universal app-connection management.

**LiteCowork:** `apps/litecoworkd/src/discover_operator.rs` should call a qualified interface through a bounded adapter (e.g. `LiteSpmClient` in an integration module); `apps/litecowork-ui/src/discover/` renders inventory/plans/results. Capability assignment remains in the existing Capability/Trust stores. Do not write LiteSPM tables from LiteCowork. Add typed service/operation schemas to existing `docs/schemas/` after upstream negotiation.

**Current blocker:** source currently says package service not qualified; there is no production-qualified end-to-end host interface. This LLD resolves ownership and required behaviors, **not the upstream protocol nor an integration acceptance gate**.
