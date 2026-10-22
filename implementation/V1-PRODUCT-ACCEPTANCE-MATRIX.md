# LiteCowork V1 — Product-to-Implementation Acceptance Matrix

**Status:** active release-evidence crosswalk. **Not a pass certificate.**  
**Sources:** [ten-area contract](../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [UI contract](../docs/V1-UI-SCREEN-CONTRACT.md), [LiteSPM interop](../docs/LITESPM-LITECOWORK-INTEROP-LLD.md), [native HorizonCode](../docs/V1-HORIZONCODE-NATIVE-AGENT-LLD.md), [existing qualification](QUALIFICATION-MATRIX.md), [served routes](served-route-inventory.json).

## Truth labels

- **TARGET:** normative HLD/LLD only; no served behavior implied.
- **CODE:** compiled/unit exercised code; may be unreachable from real app.
- **SERVED:** authenticated route and storage/source path mounted; provider/runtime may still be unusable.
- **INTEGRATED:** real provider + desktop + persisted path proven for exact version.
- **QUALIFIED:** CODE, SYSTEM and owner USER evidence on exact OS/build/version, including negative recovery.
- **BLOCKED:** required V1 flow has no qualifying end-to-end evidence. A future pass must link evidence rather than overwrite this label without explanation.

## Required journeys and current release verdict (2026-10-22 review)

| Area | User-visible release journey | Domain/source owner | Evidence / release state |
|---|---|---|---|
| Chat | Fresh install → choose qualified native lead → authenticate → send/stream/stop → reload persisted history | E03-S01/S02; `conversation_operator.rs`, `ConversationsPage.tsx` | **BLOCKED** — composer Send disabled and native session admission refuses |
| Native agent | Detect, configure native account/options, start a real native session, preserve native existing config | E03-S01/S06–S11, Agent Fabric | **BLOCKED** — descriptor/profile modules are not qualified sessions |
| Coworker | Quick Create → first usable chat → scoped context/worker/grant → restart | E08-S01/S03; domain-responsibility | **BLOCKED** — underlying real chat/memory path unavailable |
| Computer/Workbench | Real browser/computer/PTY work with takeover, return and observation | E07-S02/S05, E08-S04/S07 | **BLOCKED** — full provider/platform SYSTEM not qualified |
| LiteSPM | Observe native MCP → preview adopt → unchanged config → assign/grant → invoke → unmanage | E04-S02/S06, E10-S01; LiteSPM service | **BLOCKED** — upstream machine contract and real integration not qualified |
| Task | Request durable outcome → accept plan → admit Attempt → perform/verify artifact → restart safely | E03-S03, E04-S03/S05, E07-S00 | **BLOCKED** — admission/lease proof producers not integrated |
| Multi-agent | Chief delegates one bounded worker Step → verified integration/handoff | E05-S01–S05 | **BLOCKED** — native/host child execution and safe lead switch not qualified |
| Automation | Schedule → exact occurrence → Task → settle/notify → recover from sleep/restart | E09-S01–S05, Runtime | **BLOCKED** — Task execution is still unqualified, recurring trigger hosting and OS notification delivery do not exist, and the settlement producer is CODE (storage + served route tests) with no owner USER evidence |
| Memory | Scoped source → grounded retrieval → eligible learning → review/revoke → no cross-scope leak | E06-S02/S03, E08-S03 | **BLOCKED** — full producer/retrieval/use path unqualified |
| Artifacts | Real source → create supported doc/spreadsheet/code → preview/edit/version/export/reopen | E04-S05, E08-S07, E07-S04 | **BLOCKED** — format/provider-specific SYSTEM paths unqualified |
| Desktop/release | Installer, accessibility, backup/restore, recovery, diagnostics, performance by OS | E02-S01, E13-S01–S04 | **BLOCKED for V1 GA** — individual code slices are not whole-platform proof |

These are product-level release verdicts, not a claim that their underlying isolated source components have no passing tests.

## Risk and owner ledger

| Priority | Exact corrective item | Primary file/epic |
|---|---|---|
| P0 | Chat-first HLD + no mandatory Task/Coworker first use | `ARCHITECTURE.md`, PRODUCT/EXPERIENCE, E02/E03 |
| P0 | Real native Session/Send instead of disabled control | E03-S01/S02; `conversation_operator.rs`; `ConversationsPage.tsx` |
| P0 | Real plan/Attempt admission and durable receipts | E03-S03, E07-S00, E04-S03/S05; `storage-sqlite/src/execution.rs` |
| P0 | LiteSPM interop/versioned inventory and plan/apply authority | `LITESPM-LITECOWORK-INTEROP-LLD.md`; `discover_operator.rs`; upstream LiteSPM |
| P1 | HorizonCode dedicated AgentModule and worker conformance | `V1-HORIZONCODE-NATIVE-AGENT-LLD.md`, E03/E05 |
| P1 | Discover mounted and real package use/management UI | `App.tsx`, `src/discover/` |
| P1 | Real Browser/Terminal/Computer provider, Workbench and format writer qualification | E07/E08 |
| P1 | Scope-respecting memories and real Knowledge retrieval | E06/E08 |
| P1 | Recurrence occurrence settlement from a terminal Task outcome: producer and served route are CODE-implemented; owner USER evidence still pending | `automation_host.rs` (`settle_started_occurrences_from_task_outcomes`), `occurrences.rs` |
| P1 | Trigger-host cursor retirement on disable, plus real OS notification delivery (attempts, `notification.delivery.changed.v1`, OS bridge) | `workspace_archive.rs`-adjacent disable path, `notification.rs` |
| P1 | Old nonexistent implementation-file references removed or target-labelled | `CONNECTORS-WORKFLOWS.md`, `LEARNING.md`, review packets |
| P1 | Fix docs claiming LIVE for unserved operation/events | `API.md`, `EVENTS.md`, `STORAGE.md`, generated route inventory |
| P2 | Consistent packaging/free local core and true format capability labels | PRODUCT, SCOPE, ARTIFACTS, E13 |

## Definition of DONE

For every product-area journey: (1) one source-authority command and service; (2) actual route/adapter; (3) state change/readback and idempotency; (4) real qualified external provider/OS; (5) documented negative, crash and cancellation cases; (6) user-visible truth/error/loading states; (7) CODE + SYSTEM + owner USER evidence, exact versions and links.

A green lint/unit suite by itself changes a row to CODE at most. A mock Tauri surface is not SYSTEM. An agent catalog or ACP handshake is not a native session qualification. Unknown provider billing must remain unknown. When a cell cannot run on current hardware mark BLOCKED with the explicit reason; do not silently claim cross-platform support.

## Follow-up implementation runs

Implementation story details and acceptance IDs remain in `backlog.json`, `epics/E*.md`, `reviews/*.md`. This matrix does not silently change their states. Reconcile after real source changes, with all platform/provider versions and named defects in the PR. No calendar release date is inferred.
