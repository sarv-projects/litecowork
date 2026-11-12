# LiteSPM ↔ LiteCowork — Local Package Management and Invocation Interoperability LLD

**Status:** LiteSPM upstream local stdio bridge is **WIRED** in its own repository; LiteCowork integration remains **TARGET / not qualified**.
**Owners:** LiteSPM owns package/catalog/inventory/config-mutation authority; LiteCowork owns Workspace/Coworker/Task authorization, execution and evidence.
**Upstream research snapshots:** this LLD includes observations from `111fd9b8` (checked 2026-11-05), `f77a3a3` (checked 2026-11-06) and `84d5fab` (checked 2026-11-09). None is an owner-approved LiteCowork integration pin.

**2026-11-12 boundary correction:** root ARCHITECTURE.md and AGENTS.md make the published local MCP stdio bridge the LiteCowork integration boundary. This LLD's direct daemon IPC/client recommendations and service-descriptor proposal are non-normative research and are not authorized for implementation. The documented MCP tool count conflicts (12 tools in the 84d5fab study, 13 at 111fd9b8); re-read exact server/tool schemas at the owner-approved pin. Runtime/process ownership, process identity, approval and recovery semantics remain unresolved in Feature 013. See [LITESPM-INTEGRATION-STUDY-2026-11.md](LITESPM-INTEGRATION-STUDY-2026-11.md) and [Feature 013](../specs/013-litespm-package-lifecycle-bridge/README.md).
**Prerequisites:** Pin a LiteSPM release/commit and qualify the exact MCP tool schemas, host-target semantics, peer-auth behavior, plan/approval protocol, operation receipt, idempotency and UNKNOWN recovery before enabling any LiteCowork adapter. The documented launch surface is `litespm bridge stdio --host <host-id>`; host-specific prompt behavior is not uniform. Do not scrape CLI output or guess RPC/tool names.
**References:** [Product](V1-PRODUCT-RELEASE-CONTRACT.md), [Capability Fabric](CAPABILITY-FABRIC.md), [Capability Invocations](CAPABILITY-INVOCATIONS.md), [Agent Control](AGENT-CONTROL.md), [Trust](TRUST.md), [upstream STATUS.md at pinned commit](https://github.com/sarv-projects/LiteSPM/blob/111fd9b8e84573f8fce76b6374b4ff9639502539/STATUS.md), [upstream AGENTS.md host UX](https://github.com/sarv-projects/LiteSPM/blob/d7dada2b5447a5f4a97f0018b0d901411409e123/AGENTS.md).

## Current product-site and upstream bridge evidence (checked 2026-11-05)

The current user-facing catalog UI referenced by the product owner is
[LiteSPM](https://lite-spm-nine.vercel.app/#/). It is a browser site, not an RPC endpoint.
The upstream README/STATUS snapshot still names a legacy Cloudflare Pages origin; that
deployment reference is not a separate service API. LiteCowork selects no machine HTTP
endpoint and does not call, scrape or infer API routes from the catalog UI.

At upstream commit `111fd9b8e84573f8fce76b6374b4ff9639502539`, STATUS.md marks
`internal/bridge` (the 13-tool stdio shim) **WIRED**. The upstream status describes
`request_install` reaching `install.execute` for supported skills, MCP servers and
pinned-source plugins, and `get_invocation`/`cancel_invocation` reaching the invocation
registry. The bridge is launched as a host subprocess via
`litespm bridge stdio --host <host-id>`; it is a thin local client of the LiteSPM daemon.
The status also describes automatic daemon cold-start and retry after a dead-socket
handshake. These are upstream implementation assertions, not LiteCowork integration
evidence.

The host UX is version-specific. The upstream notes identify OpenCode v2.2.5 as exposing
the prompt under `/litespm:marketplace`, while Qwen Code 0.14.4 does not map MCP prompts
to slash commands and instead exposes the MCP tools under `/mcp`. Do not promise a
universal `/marketplace` interaction across native agents.

The upstream daemon local-IPC contract distinguishes transport permissions from independently
verified peer identity. These daemon endpoint observations are not LiteCowork's auth
contract while the product boundary is MCP stdio. Runtime/process ownership must provide
the process-identity evidence for each OS; stdio and MCP metadata alone do not prove it.

The upstream README lists dynamic catalog and host figures, but counts are time-bound and
must not become LiteCowork promises. The UI consumes current package-level installability,
provenance and per-host compatibility evidence; listing volume is not installation
coverage or security qualification.

## 1. Problem and ownership

Without LiteSPM, a user repeats package setup across Codex, Claude Code, OpenCode and other compatible agents. LiteSPM shall manage both eligible **existing native capabilities** and new installs without forcing reinstallation or flattening incompatible host semantics. Its inventory is the source of **observed package state**, not a substitute for agent-native runtime discovery.

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

Use only LiteSPM's **published, pinned MCP stdio bridge** as the integration boundary identified by root ARCHITECTURE.md. Do not invent a LiteCowork `LiteSPMServiceDescriptor`, daemon endpoint, direct RPC method or a new authentication challenge. LiteCowork validates the negotiated MCP protocol/server information and exact bounded tool manifest against the published pin. The Runtime/process owner is responsible for the approved launch/session lifecycle and per-OS process-identity evidence; MCP stdio metadata alone does not prove OS identity. If that evidence or a needed operation is unavailable, keep the integration or verb unavailable. No shell command may be built from arbitrary package input.

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

LiteSPM owns the final published MCP tool names, schemas, error vocabulary and protocol version. This document must be reconciled with that upstream publication before a real client is marked integrated. Version-incompatible clients fail closed; daemon method names recorded in research are not substituted for MCP tool names.

## 5b. Historical upstream package-taxonomy observation — pinned 2026-10-22

This subsection preserves the package taxonomy and installability observations captured at the cited October commit. It is not the current LiteSPM bridge-status snapshot; use the 2026-11-05 pinned STATUS/AGENTS evidence above for current bridge wiring and host UX. Recheck the current upstream schema before relying on any package-kind or installability enum.

Section 5 requires this document to be reconciled with LiteSPM's own publication before a
client is marked integrated. The upstream repository was read (not modified) at commit
`da80e5ee902bee0aa89898d541ebe9b1205a6b8a`, dated 2026-10-22. Everything below is an
**observation of upstream-owned facts**, cited to upstream files. LiteSPM owns them; this
document records, and never redefines, them.

### Package kinds, and the connector deferral

`internal/domain/models.go` defines nine listing kinds: `plugin`, `mcp`, `skill`,
`connector`, `agent`, `rule`, `hook`, `tool`, `lsp`. Two facts matter for this design:

1. **`connector` is explicitly marked upstream as `// deprecated: deferred to v2`.** It is
   also absent from the published `kind` enum in `schemas/listing.schema.json`, which lists
   only `plugin | mcp | skill | agent | rule | hook | tool | lsp`.
2. **Installability is a provenance ladder, not a feature flag.** Upstream defines
   `discovery_only` (searchable metadata, no proven artifact — heuristic ingestion such as
   awesome-lists can never claim more), `metadata_verified` (an authoritative upstream
   manifest proves a version and launch line) and `runtime_verified` (the launch line was
   observed to start). Only some kinds are wired for install at all: `mcp` at
   `metadata_verified` or better, `skill` and `plugin` at `runtime_verified` or better;
   `connector`, `agent`, `rule`, `hook`, `tool` and `lsp` are not installable
   (`internal/domain/canonical_test.go`).

**Consequence for our connector breadth design.** A "connector" cannot be added to LiteSPM
today. What can be added is an **MCP server listing**, a **skill listing**, or a **plugin**.
Any LiteCowork feature that promises "add a connector" must therefore be honest that it is
offering an MCP-backed capability (or a skill), and that a first-class connector kind is an
upstream v2 matter outside LiteCowork's authority.

### How a package is actually added, upstream

Observed path, in the order upstream's own schemas imply:

1. **Author the artifact.** A skill is a directory with `SKILL.md`; an MCP server is a
   launchable command line with its configuration.
2. **Describe it as a listing** (`schemas/listing.schema.json`). Required fields are
   `schemaVersion`, `id`, `kind`, `name`, `summary`, `categories`, `source`, `versions`,
   `status`; optional fields carry the trust signal — `publisherClaim`, `provenance`,
   `installability`, `compatibilitySummary`, `verificationSummary`, `rawMetadataRef`.
3. **Supply `source.schema.json` and `version.schema.json`** for provenance and the exact
   version being offered.
4. **Publish a catalog release** (`schemas/catalog-release.schema.json`): a sharded
   listing/version index plus a `manifest.json` describing the shards.
5. **LiteSPM resolves and installs**: it produces an `install-plan.schema.json` plan, honours
   the project manifest (`schemaVersion`, `projectName`, `defaultScope`, `requires`,
   `policy`, `lockRequired`, `bindingMode`), records the result in a lockfile and a
   provenance ledger, and confines every written path.

### Upstream already installs skills across harnesses

`internal/skills/` installs `SKILL.md` directories **by copy** into each agent's own skills
directory, deliberately not by symlink, so an installation can be inspected and removed; it
takes its per-agent directory map from the ecosystem's de-facto reference (vercel-labs
`skills`), and it carries a provenance ledger plus path-confinement and name-safety checks.

**Consequence for our learning design.** [LEARNING §15b](LEARNING.md) must not propose a
second installer. A demonstration-derived or imported skill ends as a `SkillProposal`, and
**LiteSPM performs the cross-harness installation**; LiteCowork's own responsibility stops
at provenance, review, authority declarations and the assignment of the installed capability
to a Workspace/Coworker.

## 6. Plan/apply transaction and crash recovery

1. The approved Runtime/process owner supplies a local stdio MCP session; LiteCowork validates initialization, server info, tool manifest and process-identity evidence against the pinned bridge profile. The user selects the exact package/host/scope only after the profile is ready.
2. Read observed/native state and host compatibility. Present source, origin and exact changes.
3. LiteSPM produces **read-only** mutation plan: target locator, pre-image hash, new image (or ownership record only), conflicts, replacement/rollback and requirement for user consent.
4. User approves a particular plan digest/expiry and host/scope. Approval to **manage package config** is separate from LiteCowork execution grants.
5. Apply only through an exact published MCP tool schema that preserves the stable idempotency key and owner-approved plan identity; LiteSPM revalidates freshness and preconditions, records recoverable operation intent, executes each owned mutation, commits durable operation receipt and read-back observation. If the published tool surface cannot preserve these semantics, the mutation verb remains unavailable.
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


## External desktop plugin-manager comparison - 2026-10-31

The [Desktop Reference Implementation Audit](DESKTOP-REFERENCE-IMPLEMENTATION-AUDIT.md) reviews DeepSeek Harness's profile-owned plugin management and its Electron Desktop command path. Treat it as a comparison, not a new LiteCowork package owner. LiteSPM remains authoritative for catalog, inventory, install/update/remove, host registration and provider process lifecycle; LiteCowork Core remains authoritative for scoped invocation grants and Effects. In particular, do not copy the DSH same-Node-process plugin execution model for untrusted community packages, and do not introduce a second package database or package writer.

---

## Appendix A — Upstream parity requests (PROPOSAL — for owner review, not sent upstream)

*Drafted 2026-11-06 from a read-only study of `sarv-projects/LiteSPM@f77a3a3eeb38396788cc31240105cf2e0321a205` (main, shallow clone at `/home/forge/refs/litespm`; LiteSPM was not built, run or modified). Every "current state" reference below was read in that checkout. Claims are tagged **[verified-by-reading]**, **[verified-by-command]** or **[inferred]**. Nothing here has been filed, sent or published upstream; it is a request list to agree with the LiteSPM owner before any of it is relied on.*

The V1 one-tap goal (install/adopt/control an MCP server, skill or plugin across agents from LiteCowork) needs six upstream surfaces that HEAD does not publish today.

**A1. Daemon method to record a caller-authorized approval (blocks a true one-tap).**
- Current state: the daemon registers 20 methods plus `daemon.handshake` (`cmd/litespm/main.go` L.2794–L.3763; `internal/ipc/server.go` L.74); none creates or resolves an approval. `install.execute` requires a consumed approval bound to the plan (`cmd/litespm/install_authz.go` L.117), and approvals are recorded only by the human CLI — `runApprove` (`cmd/litespm/install_authz.go` L.346), `recordHumanApproval` (`#L245`) — i.e. `litespm approve <plan-id>`. **[verified-by-reading]**
- What LiteSPM would need to expose: either an `approvals.record` method (one-shot, with the same consumption/refund semantics as `authorizeInstallExecute`'s grant, daemon as consumption authority), or a documented non-interactive `litespm approve` mode with machine-readable output. **[inferred: shape]**
- Why LiteCowork needs it: the tap closes inside `litecoworkd`; without this, the approve step must spawn the LiteSPM CLI and its interactive behaviour is unproven. **[inferred: CLI behaviour — never executed, per read-only constraint]**

**A2. Operation identity for install/adopt/remove (needed by the recovery contract).**
- Current state: `invocation.get`/`invocation.cancel` exist for provider invocations (`cmd/litespm/main.go` L.3618, L.3640); there is no `operation.get`/subscribe for package operations. Recovery (this doc, L.182) expects querying "the original operation identity" after a lost response. **[verified-by-reading]**
- Requested: `operation.get` by plan/install id returning terminal state and receipts, or an explicit statement that `receipts.jsonl`, `installs` and `audit_events` are the supported query surface. **[inferred: preferred shape]**

**A3. Reconcile and unmanage.**
- Current state: no reconcile IPC, and no `unmanage` verb exists anywhere in code — "unmanaged" appears only as copy-refusal language (`cmd/litespm/copy.go` L.1114, L.1122). This doc expects three-way reconciliation (L.101, L.187) and unmanage that preserves the originally present native entry (L.102). **[verified-by-reading]**
- Requested: the methods, or an upstream scope decision that V1 is adopt + remove only, with reconciliation remaining CLI/operator work. **[inferred: decision]**

**A4. Advertised-operations service descriptor.**
- Current state: only `daemon.handshake` exists, and it gates every other method (`internal/ipc/server.go` L.294). No payload advertises supported operations, limits, auth challenge or health as L.90 requires. `daemon.status`/`daemon.health` are listed retryable in the client (`internal/ipc/client.go` L.197) but have no server handler — dead surface today. **[verified-by-reading]**
- Requested: descriptor fields in the handshake payload or a `daemon.describe` method; register or remove the two status methods. **[inferred: shape]**

**A5. Agreed meaning of health/qualification.**
- Current state: `doctor.run_checks` explicitly does not start or probe MCP servers ("reports configuration and storage health, not runtime health", `cmd/litespm/help.go` L.405); `provider.probe` is the runtime path (`cmd/litespm/main.go` L.3536). L.104 expects observed readiness. **[verified-by-reading]**
- Requested: one documented qualification definition the bridge can hand LiteCowork (config-health vs runtime-health vs both), so `connect-or-install` can move past `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED` honestly. **[inferred: product decision]**

**A6. Signature posture for the catalog pointer.**
- Current state: pointer→manifest→listings digest verification is enforced; the cosign bundle path exists but the published pointer carries no `.sigstore.json` today (`STATUS.md` L.61), and `LITESPM_REQUIRE_CATALOG_SIGNATURE=1` turns absence/unverifiability into a hard refusal (`internal/catalog/client.go` L.488–L.489, L.550). **[verified-by-reading]**
- Requested: decision — does LiteCowork require the signed pointer and refuse unsigned catalogs, and will upstream publish the bundle? **[inferred: policy]**

## Appendix B — Reconciliation note against pin `111fd9b8` (PROPOSAL — for owner review)

*Same basis as Appendix A. Purpose: record what in this LLD is stale now that upstream has moved, so the next edit re-grounds on a verified revision instead of the pinned one.*

**B1. Pin drift is material and recent.**
- This LLD pins `sarv-projects/LiteSPM@111fd9b8…` (L.5, checked 2026-11-05). HEAD on 2026-11-06 is `f77a3a3…`, **37 commits ahead, 0 behind** (`gh api repos/sarv-projects/LiteSPM/compare/111fd9b8…f77a3a3`, status `ahead`). **[verified-by-command]**
- Those 37 commits include, from commit subjects: "security: fix audit findings F1-F4", host-writer fixes (JSONC/TOML merging, apply guards, lost-backup refusal), bridge prompt honesty changes and the wizard unification; further `docs(audit)` backlog entries. **[verified-by-command: subjects; inferred: per-fix details not re-read]**
- In particular, the F3 commit makes `host.apply_setup` refuse a caller-supplied `binaryPath` that does not resolve to the running binary. Any design text that assumed binaryPath injection must be re-checked against post-pin code. **[verified-by-command: commit message; code not re-read line-by-line]**

**B2. `connect-or-install` is served but fails closed — the "Current blocker" paragraph (L.224) remains accurate at this revision.**
- LiteCowork side: route present in the served inventory (`docs/schemas/served-route-inventory.json` L.777) and the failure is explicit, reason `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED` (`apps/litecoworkd/src/discover_operator.rs` L.369; constants at `crates/storage-sqlite/src/discover.rs` L.37, `crates/storage-core/src/skill.rs` L.62). **[verified-by-reading]**
- LiteSPM side: the only agent-facing shell is the 13-tool MCP stdio bridge (`internal/bridge/shim.go` L.71–L.174); a daemon-level lifecycle interface for an external supervisor like LiteCowork is exactly what Appendix A requests. **[verified-by-reading]**

**B3. Corrected claim: do not carry forward "ARCH/38 still calls `copy` DESIGNED".**
- At `f77a3a3`, ARCH/38's header marks `copy` **IMPLEMENTED** (§3/§4/§5 header lines 3–6; §5 heading at L.217: "`copy` — cross-agent porting (`IMPLEMENTED` — `cmd/litespm/copy.go`, `internal/porting`)"), matching the wired `cmd/litespm/copy.go`. The earlier verbal claim that the doc says DESIGNED **did not reproduce** and is withdrawn here. **[verified-by-reading]**
- Related, still true and consistent: ARCH/38's own tree note says "package update is DESIGNED" (`ARCH/38-CLI-PRODUCT-SURFACE.md` L.75), matching the code — none of the daemon's 20 methods updates an installed capability. **[verified-by-reading]**

**B4. LLD surface names vs actual wire names (fold into A1–A4 when this doc is next edited).**
- Semantic ops in §4/§6 map to actual methods: plan → `resolver.prepare_plan`, apply → `install.execute`, adopt → `adopt.execute`, remove → `install.remove`, inventory → `tools.list`/`skills.list` (+ `litespm list --json`), describe → `catalog.get_item`/`capabilities.describe`, invoke → `provider.invoke`. Reconcile, unmanage, operation-get/subscribe and approval recording have no wire method. **[verified-by-reading]**
- §10's "extend existing `cmd/litespm/adopt.go`, `list.go`, `copy.go`" remains valid — all three exist and are wired at this revision. **[verified-by-reading: file presence; inferred: extent of wiring beyond copy]**

**B5. Re-pin instruction.**
- Before any further integration edit, re-pin this LLD against a stated LiteSPM revision and re-run the Appendix A checks; upstream moved 37 commits in one day, so a pin's age in days is not a proxy for stability. **[inferred: process recommendation]**

