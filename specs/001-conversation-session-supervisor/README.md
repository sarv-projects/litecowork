# 001 - Conversation Session Supervisor and Native Turn Execution

- **Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
- **Revision:** 1.1 | **Updated:** 2026-11-09 | **Owner acceptance:** B-01 to B-10 accepted 2026-11-09 via [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md); first route to qualify is OpenCode v2.0.32 on Windows x64 (B-02 rider).
- **Implementation state (source-checked 2026-11-15):** partial CODE/SERVED foundations; no live product-managed OpenCode journey and no QUALIFIED row. T02 is PARTIAL pending A-93 (storage response-commit contract mismatch); T08 is PARTIAL: native-Windows SQLite regressions cover prior-incarnation recovery of an orphan STARTING session and a RUNNING turn with an ACTIVE session, binding cleanup, idempotency and preservation of USER input without an invented answer. The crash-after-final-commit case, full crash-window matrix, and exact failed-turn UI/Retry projection remain untested. T13 is PARTIAL pending A-94 (the accepted `agent_session_id` correlation is absent from the producer/schema). T06, T10, T11, T16-T19 and T26 have source/test evidence; T07 remains PARTIAL for failure-race coverage; T20 has CODE for closed dispatch-failure classes, exact-attempt settlement (including blocking-worker failure), profile-lookup error settlement, bounded transient retry, captured structured-diagnostic writer privacy and recovery fallback. Binding-lookup and exhaustive failure-stage recovery, full service-boundary extraction, shared-host/StopUnconfirmed lifecycle integration, panic-hook/external collector checks and provider qualification remain open. T24 remains fixture-driven pending exact live capture. T01, T03-T05, T14, T21-T23, T25 and T27 are PARTIAL; T12 is BLOCKED pending a real provider input event and approved B-06 mapping; T09 and T28 are NOT STARTED. Migration v29 adds dispatch intent, response identity, usage and single-flight guards; v30 adds reported activity. The dated source cross-check remains [historical reference](../../docs/audit/FEATURE-001-CURRENT-CROSSCHECK-2026-11-13.md); TASKS.md, `CURRENT-RUN.md` and docs/IMPLEMENTATION-STATUS.md record current evidence. This status does not qualify the packaged desktop route.
- **Owners:** Conversation, Agent Runtime, Storage, Operator/API, Desktop UI, Security/Trust and Product.

## User outcome

A user can send an ordinary LiteCowork Conversation message through the exact selected native agent, see truthful progress, stop/retry safely and reload the durable answer after restart, without creating a Task or Coworker.

## Current truth

The active source contains an OpenCode-only supervisor path and code-level durability, single-flight, stop/retry, bounded-context, and attachment foundations. Current task-level implementation and exact evidence are tracked in TASKS.md and docs/IMPLEMENTATION-STATUS.md. The host OpenCode v2.0.32 CLI is installed and its auth list reports stored provider entries, but no product-managed authenticated send/stop/retry/restart journey or live input-request event capture has been completed. The remaining lifecycle and qualification gaps stay release-blocking.

By owner approval on 2026-11-09 (see [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md)), this package also folds the audit's cowork-visibility tasks P-00, P-01, P-02, P-04 and P-09 (001-T23 to 001-T27): a redacted OpenCode v2.0.32 turn fixture, the OpenCodeEventNormalizer, transient activity frames with REPORTED records, stick-to-bottom autoscroll, and native permission prompts surfaced inline and in Needs You (jointly with Feature 002). Source: [docs/UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md](../../docs/UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md) section 9.

## Package

- [SPEC.md](SPEC.md) - target HLD/LLD, decisions, security and EARS acceptance.
- [PLAN.md](PLAN.md) - phases, dependency DAG, risks and gates.
- [TASKS.md](TASKS.md) - reviewable implementation tasks and blockers.
- [DATA-MODEL.md](DATA-MODEL.md) - reused records, implemented result-write boundary and remaining durability gaps.
- [RESEARCH.md](RESEARCH.md) - sources, verified evidence, alternatives and open questions.
- [WIRING.yaml](WIRING.yaml) - component/owner/transaction graph.
- [contracts/](contracts/README.md) - API/event and failure/recovery contract use.
- [Native permission flow design](contracts/native-permission-design.md) - T27 owner design; Runtime-local correlation, served snapshot GET, target reply path, adapter-owned provider call, Needs You and recovery (not integrated or qualified).

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [Product](../../docs/PRODUCT.md), [V1 release](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Agent Control](../../docs/AGENT-CONTROL.md), [Agent Fabric](../../docs/AGENT-FABRIC.md), [Native Composer LLD](../../docs/NATIVE-COMPOSER-LLD.md), [State Machines](../../docs/STATE-MACHINES.md), [Services](../../docs/SERVICES.md), [Storage](../../docs/STORAGE.md), [Events](../../docs/EVENTS.md), [API](../../docs/API.md), [Schemas](../../docs/SCHEMAS.md), [Trust](../../docs/TRUST.md), [Failure Recovery](../../docs/FAILURE-RECOVERY.md), [Testing](../../docs/TESTING.md).

