# LiteSPM integration study (2026-11-09)

**Status:** historical upstream research snapshot, not an approved LiteCowork integration pin or owner contract. Authority for LiteCowork behavior stays with ARCHITECTURE.md, AGENTS.md section 6 and the accepted packages. Authority for LiteSPM behavior stays with the LiteSPM repository's own ARCH and STATUS documents, cited below by file and line.
**Source:** [sarv-projects/LiteSPM](https://github.com/sarv-projects/LiteSPM) cloned read-only into `repo_take/LiteSPM` (gitignored, never imported, per AGENTS.md section 7). HEAD `84d5faba75bc621a651e0cec0f831e936bb2082d`, `git describe` = `v0.6.0-143-g84d5fab`, dated 2026-11-09. npm package version `0.6.0`. License Apache-2.0. Go project, 1,039 tracked files, 426 Go files, with a 40-document `ARCH/` set and a `STATUS.md` truth baseline.
**Why this exists:** Feature 013 previously guessed at LiteSPM transport and tool names. This study preserves observations from one upstream commit for research only. The exact published MCP schemas must be re-read and pinned when the owner lifts the deferral; neither daemon internals nor marketplace/CLI output is a LiteCowork contract (AGENTS.md section 6).

## 0. Erratum (2026-11-12)

The original study's recommendation in section 3 to bind LiteCowork directly to LiteSPM daemon IPC was not an owner decision and conflicts with current root [ARCHITECTURE.md](../ARCHITECTURE.md) and [AGENTS.md](../AGENTS.md), which identify LiteSPM's published local MCP stdio bridge as the integration boundary. Sections 2 and 3 below describe upstream daemon internals for research only. They do not define LiteCowork transport, API names, identity checks or process launch. Feature 013 must consume only the exact pinned published MCP schemas through a Runtime-owned process/session boundary. The 2026-11-09 source snapshot and its 12-tool count remain historical evidence; re-read and pin the published contract before promoting any tool mapping. Owner-approved mutations, idempotency and recovery must be verified tool by tool; unsupported lifecycle operations stay unavailable.

## 1. What LiteSPM actually is

At this research commit, LiteSPM is a standalone local-first package manager for AI-agent capabilities with its own daemon, SQLite state, secret vault, catalog index and MCP bridge. Those upstream internals do not make daemon IPC LiteCowork's integration boundary: LiteCowork's boundary is the exact published local MCP stdio bridge, launched and identified by the owning Runtime process contract. LiteSPM's six-state evidence vocabulary (`DESIGNED`, `IMPLEMENTED`, `WIRED`, `TESTED`, `VERIFIED`, `SHIPPED`) is an upstream observation only; it must not be translated into LiteCowork qualification without evidence.

LiteSPM's own boundary statement matches LiteCowork's ownership split exactly: "LiteSPM's hosted catalog service is strictly a discovery index. It is never a secret store, execution environment, MCP proxy, or authentication authority. Downstream service credentials and runtime tool execution remain strictly on the user's local machine or with the user's chosen remote provider" (`ARCH/05-SECURITY.md:14`). This is the same line LiteCowork draws in AGENTS.md section 6: LiteSPM owns package discovery and lifecycle; LiteCowork owns scoped authorization for use.

## 2. Upstream LiteSPM interfaces observed at the research pin

LiteSPM exposes daemon-local IPC and an MCP stdio bridge. The earlier recommendation to bind LiteCowork directly to daemon IPC is superseded by the 2026-11-12 erratum; the daemon details below are research evidence, not a LiteCowork contract.

### 2.1 Upstream daemon transport (research only)

- **JSON-RPC 2.0 over line-delimited frames**, newline-terminated, `MaxMessageSize = 16 MiB`, sequential integer request ids minted by the caller, id-less notifications (`ARCH/11-LOCAL-RUNTIME-IPC.md` section 3; `internal/ipc/protocol.go:25-26,110-166`).
- **Windows:** named pipe `\\.\pipe\litespm-daemon-<hex>` where `<hex>` is the first 12 hex chars of `SHA256(username)`; security descriptor `D:(A;;GA;;;OW)` grants the object owner only; the OS rejects a foreign SID before `Accept` (`ARCH/11` section 2.1; `internal/ipc/transport_windows.go:13-19`).
- **Linux/macOS:** unix socket `<RuntimeRoot>/litespm.sock`, directory `0700`, socket `0600`, with an authoritative kernel peer-credential check (`SO_PEERCRED` on Linux, `LOCAL_PEERCRED` on macOS) that refuses a peer uid different from the daemon's with `-32001 CodeUnauthorized` before any handler runs (`ARCH/11` section 2.2; `internal/ipc/transport_linux.go`, `transport_darwin.go`, `server.go`).
- This is the same OS-authenticated local-IPC posture LiteCowork's own daemon uses (peer uid check, owner-only DACL). The two trust boundaries are compatible; LiteCowork dials LiteSPM's endpoint as a same-user local client.

### 2.2 Protocol and handshake

- Handshake method `daemon.handshake`; wire types `HandshakeParams` / `HandshakeResult` (`internal/ipc/protocol.go:55-69`). Request fields: `clientVersion`, `clientKind` (`"cli"` or `"bridge"`), optional `hostId`, `pid`. Response: `daemonVersion`, `protocolVersion`, `pid`, `peerAuth {verified, method?, uid, pid?, reason?}`.
- **Protocol version string is `2026-07-28`** (24 references across `internal` and `cmd`; `ARCH/06-API-CONTRACTS.md:211`). The MCP side also negotiates `2026-07-28` in `initialize` and reports the real `buildinfo.Version` (`ARCH/14-BRIDGE-PROVIDER-MCP.md` section 1.2).
- Historical upstream observation only: the daemon handshake is defined but **not yet enforced or called by any production client** (`ARCH/11` section 3.1). It is not a LiteCowork client contract. Feature 013 must validate the published MCP handshake and server/tool schemas at the owner-approved pin; OS process identity belongs to the LiteCowork Runtime owner.
- Cancellation is `$/cancelRequest` with `{id}`, implemented both directions (`ARCH/11` section 3.2).

### 2.3 The 19 daemon methods and the 12 bridge tools

The daemon registers `daemon.handshake` plus 19 application methods (`ARCH/06` section 3.2; `cmd/litespm/main.go:1956-2744`): `tools.list`, `catalog.search`, `catalog.get_item`, `resolver.prepare_plan`, `install.execute`, `install.remove`, `skills.list`, `skills.load_body`, `skills.read_resource`, `capabilities.search`, `capabilities.describe`, `provider.probe`, `provider.invoke`, `invocation.get`, `invocation.cancel`, `host.detect_config`, `host.apply_setup`, `doctor.run_checks`, `system.status`.

At this historical source snapshot, the bridge shim advertises 12 MCP tools and maps them to daemon methods (`ARCH/14` section 1.2); an older snapshot in the interop LLD records 13, so neither count is approved. The names below describe upstream implementation only. LiteCowork must bind only to the published MCP tool names and schemas selected at an owner-approved pin; it must never call the daemon methods directly or derive wire names from them.

Status to respect (from LiteSPM's own STATUS, not assumed): `catalog.search`, `catalog.get_item`, `resolver.prepare_plan`, `install.execute` (for skills, MCP servers and pinned-source plugins), `install.remove`, `skills.*`, `invocation.get`, `invocation.cancel` are `WIRED`; `capabilities.search/describe` and `provider.invoke` "resolve" (reach the handler) with `provider.invoke` synchronous by contract; the providers table has no non-test writer, so nothing autostarts in production; provider health/circuit-breaking is `DESIGNED` only (`ARCH/14` sections 1.2, 2, 5).

### 2.4 Install plan and approval (binds cleanly to LiteCowork's plan-digest rule)

- `resolver.prepare_plan` builds, hashes and persists an `InstallPlan`; `install.execute` re-verifies it (`ARCH/06` section 2). `planId` matches `^plan_[0-9A-Za-z]{26}$`.
- **Upstream observation:** `planHash` is `sha256:` over RFC 8785 canonical JSON of the execution fields (`ARCH/06` section 2.3). This resembles LiteCowork's digest-bound approval invariant, but the published MCP tool schema and approval path at this snapshot are not proven to carry that digest. Feature 013 must verify the exact published contract; until then no install/mutation is available.
- The plan schema (`schemas/install-plan.schema.json`) carries `effects`, `requestedAccess`, `hostChanges`, `providerLaunches`, `sourceSnapshots`, `immutableRefs`, `selectedComponents`, `approval {approvalId, approvedBy, decision, decidedAt}`, `targetHost`, `targetScope`, `projectRoot`. LiteCowork previews these effects and host changes before the user approves.

### 2.5 Effect taxonomy and security model (aligns with LiteCowork invariants)

- LiteSPM classifies plans under a **17-action taxonomy** at this snapshot (`ARCH/15-POLICY-APPROVALS.md` section 1). A future LiteCowork UI may surface these only if the selected published MCP schema returns them with provenance; do not assume the private daemon representation is available.
- Egress is guarded centrally in `internal/egress`: HTTPS-only, SSRF refusal (RFC1918/ULA/link-local/metadata denied), credential-free URLs, bounded redirects (3 hops catalog/MCP, 5 artifact, 2 OAuth), destination re-validation per hop and rebinding-proof dialing (`ARCH/05` section 3). The remote-MCP policy adds same-origin redirect refusal. This matches LiteCowork's connector rules (TLS, SSRF, redirect policy, byte limits) in AGENTS.md section 6.
- Secrets never leave the local vault: a 32-byte master key in the OS keystore (DPAPI / Keychain / Secret Service), secret bytes AES-256-GCM in `secrets/vault.enc`, SQLite holding only `secret:<namespace>/<key>` references (`ARCH/05` section, `internal/secrets`). LiteSPM never puts a token in an agent prompt, which is the same rule LiteCowork enforces for Connections (capability 8).
- Grant drift: a capability grant binds `(capability_id, schemaFingerprint, casTreeDigest)` for local stdio or adds `(endpointOrigin, serverVersionDigest)` for remote; execution is refused on schema/tree/endpoint drift (`ARCH/14` section 4.1). LiteCowork's use-authorization should ride on top of, not duplicate, this binding.

### 2.6 Host adapters (the cross-agent write surface)

Six bespoke host adapters are recorded at this snapshot (`internal/host/`). This does not qualify a LiteCowork bridge registration or writer. OpenCode is a possible first host candidate because it is the selected first native-agent route, but Feature 013 remains deferred and no host route is approved here.

## 3. How LiteCowork integrates (reconciled with Feature 013)

The original integration proposal below is superseded; retain it only to explain the earlier design error. Do not implement it.

1. The old design called daemon-local endpoint methods directly, bypassing the published MCP bridge boundary. This is not an authorized route.
2. The old design mapped daemon methods to Discover inventory, plan and mutation routes before proving the MCP tool schemas and ownership model. Those mappings must be re-derived from the owner-approved published MCP pin.
3. The old design assumed LiteCowork could record a plan-digest approval and pass it to the daemon. Verify that the published tools preserve this authorization and recovery contract; otherwise the affected mutations stay unavailable.
4. These superseded details do not change the invariant that installation is separate from Connection, grant, assignment or invocation.

## 4. What this resolves and what stays open

Research findings, not owner resolutions:
- **Source snapshot:** `84d5fab` (`v0.6.0`) is the upstream checkout inspected here, not an owner-approved LiteCowork integration pin. Feature 013 and B-36 were owner-deferred; do not treat them as active work until that deferral is lifted. The published MCP package/protocol and exact tool schemas must then be selected and recorded.
- **Daemon method names:** those in upstream `ARCH/06` and `ARCH/11` are not LiteCowork MCP tool names.
- **Safety contract:** upstream plan/backup/transaction/restore guarantees are useful research; Feature 013 must verify which guarantees the published tool schemas expose at the selected pin.

Open, for the 013 implementation tasks (not this doc):
- The LiteSPM daemon handshake described in section 2 was not enforced at this snapshot. This is not a requirement for a private LiteCowork daemon client; MCP conformance and process identity must follow the selected published bridge and Runtime owner contract.
- `provider.invoke` is synchronous and provider health is `DESIGNED` in LiteSPM; LiteCowork cannot present health it cannot observe.
- LiteSPM's release round trip (commit, CI, Cloudflare) can move HEAD; 013 re-reads the pin at implementation time and records the exact release it qualifies against, per LiteSPM's own "a probe is evidence only for the release it names" rule (`STATUS.md`).
- A full qualification of the published MCP bridge, its exact tools, process/session owner and any lifecycle writes is future work in 013-T02/T04/T20; this study did not perform it.

## 5. Method and limits

Read-only. The repository was cloned shallowly into the gitignored `repo_take/LiteSPM`; no code was copied into LiteCowork. Every claim cites a LiteSPM file and line at HEAD `84d5fab`. Where LiteSPM's own STATUS marks a behavior `DESIGNED` or "not yet true", this study repeats that label rather than upgrading it. The marketing site figures (50 hosts, 81 skill targets, ~69k listings) are the hosted catalog's breadth and are not a claim about LiteCowork's qualified surface.
