# Research and Decision Log - LiteSPM Package Lifecycle Bridge

**Feature:** 013 | **Status:** IN_REVIEW - not owner-accepted; does not authorize implementation; not a claim of implementation or qualification.
**Checked:** 2026-11-09 - repository source and docs read; LiteSPM upstream read read-only at HEAD 84d5fab (v0.6.0) for the integration study; not built or run for this package; no web research.

## 0. Architecture correction (2026-11-12)

This file is research evidence, not the transport owner. The earlier conclusion that LiteCowork should call LiteSPM daemon-local IPC directly was not authorized and conflicts with the current root [ARCHITECTURE.md](../../ARCHITECTURE.md) and repository [AGENTS.md](../../AGENTS.md), which identify LiteSPM's published, pinned local MCP stdio bridge as the available integration boundary. Keep the direct daemon methods and OS endpoint details below only as upstream research; they are not LiteCowork API requirements and must not be implemented as a private client. Feature 013 remains DEFERRED. B-36/O-07/O-08 require owner review of the exact pinned MCP tool schemas, the Runtime-owned process/session lifecycle and identity evidence, and whether the published tool surface can support owner-approved plan/apply/reconciliation. Any operation not published and qualified remains unavailable.

## 1. Pinned external sources

| Source | Revision | License | Finding | Decision |
|---|---|---|---|---|
| REF-LITESPM [sarv-projects/LiteSPM](https://github.com/sarv-projects/LiteSPM) | `9f42b25ddfb1acedce0f2dbf9964f4a7724d622e` ([REFERENCE-IMPLEMENTATIONS.md](../../docs/REFERENCE-IMPLEMENTATIONS.md)) | Apache-2.0 at root LICENSE | Selected lifecycle authority; "its current checkout does not settle the LiteCowork-facing package API" | Prototype against its published contract only; copy no code; reject inferred wire names |
| LiteSPM at interop LLD header pin | `111fd9b8e84573f8fce76b6374b4ff9639502539` | Apache-2.0 | Upstream STATUS marks the 13-tool stdio bridge WIRED; Windows named pipe owner-only DACL with `peerAuth.verified=false` | Agent-facing ABI only; not this client's transport |
| LiteSPM at interop LLD Appendix A/B study | `f77a3a3eeb38396788cc31240105cf2e0321a205` | Apache-2.0 | 20 daemon methods + `daemon.handshake`; approvals recorded only by the human CLI (A1); no package `operation.get` (A2); no reconcile/unmanage (A3); no service descriptor (A4); health meaning unagreed (A5); unsigned catalog pointer (A6). Semantic-to-wire mapping recorded in Appendix B4 | Use only after re-pin at 013-T02 confirms the same at the chosen revision |

The three pins disagree. 013-T02 chooses one, records it here and in both LiteSPM owner docs, and marks the others historical.

## 2. Repository sources inspected

| Source | Path | Finding |
|---|---|---|
| Discover routes | `apps/litecoworkd/src/discover_operator.rs` | Search/describe served; connect-or-install always 503 `LITESPM_PACKAGE_SERVICE_NOT_QUALIFIED`; test returns UNKNOWN; correlation ID used as `checked_at` and `now` |
| Discover projection | `crates/storage-sqlite/src/discover.rs`, `crates/storage-core/src/discover.rs` | Skills + 50 host observations; `authoritative` always false; provider status hard-coded |
| Route classifier | `crates/domain-invocations/src/capability_bridge.rs` | Requires LiteSPM-managed + digest-verified identity; no caller |
| Desktop Discover | `apps/litecowork-ui/src/discover/*`, `apps/litecowork-ui/src-tauri/src/discover_bridge.rs` | Four tabs; Managed/Connections unavailable; all actions disabled; bridge supports search/item only |
| IPC primitives | `crates/operator-ipc/src/lib.rs`, `crates/windows-local-security/src/lib.rs` | Bounded body budgets; logon SID helper; pipe connect helper |
| Owner prose | `docs/LITESPM-LITECOWORK-INTEROP-LLD.md`, `docs/LITESPM-INTEGRATION-PLAN.md`, `docs/IMPLEMENTATION-MASTER-PLAN.md` section 3 and P5 | Ownership, verbs, plan/apply/recovery, LSI-001 to LSI-040, lane steps 1-7 |
| Flows | `docs/FLOWS.md` F04 (capability discovery and direct attachment), F22 (package update while Task running) | Upstream-owned update notification; Task does not restart for discovery |

## 3. Alternatives

| Alternative | Decision | Rationale |
|---|---|---|
| Parse `litespm list --json` or other CLI output | Reject | AGENTS.md section 6; CLI output is not a contract |
| Use the published MCP stdio bridge as LiteCowork's integration boundary | Current architecture boundary; conformance pending | Root ARCHITECTURE.md describes this as the upstream wired bridge. A direct Runtime-owned LiteCowork caller must be checked against exact pinned tool schemas and must keep owner approval in LiteCowork; no assumption that every lifecycle operation is exposed or safely usable follows from MCP support alone. |
| Spawn `litespm approve <plan-id>` | Reject | Interactive behaviour unproven; approval must be a typed, digest-bound daemon operation |
| Mirror LiteSPM inventory into LiteCowork SQLite | Reject | Second package DB (L-02) and stale-state risk |
| Direct LiteSPM daemon IPC with a new private client | Not authorized | The prior study selected this, but it conflicts with current root architecture. It needs an Architecture amendment and explicit owner approval before any transport or API contract can be promoted. |
| First kind `mcp` | Defer | Launches a provider process; health meaning (A5) unagreed |
| First kind `skill` by copy | Select (B-37) | Upstream copy-install with provenance ledger and path confinement; no provider process or secrets |
| Separate crate for the client | Flexible (F-01) | Faster isolated tests vs one more workspace member |

## 4. Open research

| ID | Topic | Next evidence | Owner |
|---|---|---|---|
| R-01 | Exact published MCP tool schemas for inventory, plan, owner approval, apply, operation lookup and recovery at the chosen pin | Pinned upstream MCP package/protocol schemas and negative conformance fixtures; a missing operation remains unavailable | LiteSPM owner, Capability Fabric |
| R-02 | OpenCode v2.0.32 skill directory support in LiteSPM host adapter | Upstream host table and a read-back fixture | LiteSPM owner, Agent Runtime |
| R-03 | Runtime-owned Windows process/session identity evidence for the stdio bridge | Feature 011 process lifecycle and OS-specific conformance; do not reuse LiteSPM daemon pipe research as the integration boundary | Runtime, Security |
| R-04 | Runtime-owned Linux/macOS process/session identity evidence for the stdio bridge | Feature 011 process lifecycle and OS-specific conformance; do not reuse LiteSPM daemon socket research as the integration boundary | Runtime, Security |
| R-05 | Rollback semantics on partial skill copy | Upstream install/remove tests at pin | LiteSPM owner |

## 5. LiteSPM bridge pin (P-17, folded 2026-11-09; transport conclusion superseded 2026-11-12)

Task 013-T20 folds proposed task P-17 from [REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md](../../docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md) section 8. The snapshot below records what was read at one upstream revision. It does not establish the LiteCowork transport, make the old direct-daemon design normative, or qualify every lifecycle operation. The package remains DEFERRED, and T02 must re-check the published bridge contract at the owner-selected pin before promoting schemas or tool names.

Pin status (snapshot read 2026-11-09): LiteSPM was inspected at HEAD **`84d5faba75bc621a651e0cec0f831e936bb2082d`** (`v0.6.0-143-g84d5fab`, npm `0.6.0`, Apache-2.0). That snapshot's published MCP stdio bridge and its schemas are the relevant candidate boundary. The earlier study also recorded daemon-local JSON-RPC methods and OS endpoint details; those describe upstream internals only and are not a LiteCowork integration contract. The earlier claim that LiteCowork should call those daemon methods directly is superseded by the 2026-11-12 correction above. The three source SHAs are historical observations, not an owner-approved integration pin. At 013-T02/013-T20, re-read and pin the actual published bridge package/protocol, capture its exact MCP protocol/server/tool schemas and auth/process assumptions, and mark each semantic lifecycle operation ADVERTISED or ABSENT. No LiteSPM code is copied into LiteCowork; `repo_take/` is research-only and must be preserved if present.

## DeepSeek Harness proposals (PROPOSED, owner review required)

Research note, not a decision. Source: [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159) at the pinned SHA (MIT, Copyright (c) 2026 DeepSeek). Local read-only clone `repo_take/deepseek-harness` is gitignored and never staged. Upstream paths below were checked to exist at that SHA on 2026-11-10.

Read-depth limits from the study (do not over-trust): `scripts/` about 58% read, tests about 5% read, installer UI text partly unread. See [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 0 and 15 before turning any item into a design.

- **P-43 Lifecycle-script allowlist and classified patch policy for dependencies.** Class **ADAPT**. Upstream: [scripts/runtime-patch-policy.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/scripts/runtime-patch-policy.ts), [src/project-manager.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/project-manager.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 11 and 14 (row P-43). Lesson: Applies to LiteSPM-linked dependency handling (package 013, DEFERRED) and the release dependency audit (T13 of package 019). pnpm specifics do not carry over to cargo and the LiteSPM bridge. Reuse: Pattern only, reimplement. No upstream code or text is copied or imported (AGENTS.md section 7; upstream is Electron and TypeScript, LiteCowork is Tauri, Rust and React). Where it applies: Package 013 (DEFERRED); T13 of package 019. Deferral: DEFERRED with LiteSPM (package 013).

Plugin-architecture lessons (see [plugin study](../../docs/DEEPSEEK-HARNESS-PLUGIN-ARCHITECTURE-DEEP-DIVE-2026-11.md) Section 8):
  - Capability seam and in-process plugin model: [docs/capability-seams.md](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/docs/capability-seams.md). Class **ADAPT for seams; REJECT in-process plugins and lazy client plugin loading for V1 (package 016 forbids model HTML)**. See also .agents/notes/implemented/architecture/2026-07-23-client-plugin-loading-model.md. Reuse: pattern only, reimplement; no upstream code imported.
  - Client plugin loading model: [client-plugin-loading-model](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/.agents/notes/implemented/architecture/2026-07-23-client-plugin-loading-model.md). Class **REJECT** for V1. Why: lazy client plugin loading conflicts with package 016 no-model-HTML. Reuse: none.

