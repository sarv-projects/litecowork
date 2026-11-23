# 018 - Usage, Diagnostics and Observability

- **Status:** IN_REVIEW - authored 2026-11-09 after the owner acceptance packet; B-42 is PROPOSED and remains OPEN after the 2026-11-10 owner review (A-62); does not authorize implementation and is not a claim of implementation or qualification.
- **Revision:** 0.1 | **Updated:** 2026-11-09 | **Delivery wave:** Wave 6 (gate G); normalization contract needed in Wave 1 by Feature 001 | **Master plan:** P11
- **Implementation state (2026-11-10):** Not implemented; B-42 open (T01 and T10). Migration v47 allocated only if needed.
- **Owners:** Observability, Operator/API, Desktop UI, Security/Trust, Agent Runtime and Product.

## User outcome

The owner opens Settings > Usage & Monitoring and sees honest, source-labelled usage per Workspace, Conversation, agent and day, with unknown and estimated values separated and no fabricated `$0.00`, `0 tokens` or `100%`. In Settings > About & Diagnostics the owner sees real build information, health probes, redacted logs and local crash records, and can write a previewed, allowlisted support bundle to a local file. Nothing is uploaded.

## Current truth

No usage record exists in any Rust source; the OpenCode adapter parses no token field; usage routes are target-only. The quota observation route is served and always returns `null`. Daemon diagnostics are 12 `eprintln!` calls; there is no structured log, crash record or support bundle. Settings has four categories and neither Usage nor About. See [SPEC.md](SPEC.md) section 4.

Under this package's IN_REVIEW status, proposed task P-18 is folded in as 018-T18: generate `operator-api.ts` and the TypeScript validators from the Rust wire types (NeuralAgent pattern) and add a drift gate, so the wire contract has one source of truth (audit finding X-04). Source: [docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md](../../docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md) section 8.

## Package

- [SPEC.md](SPEC.md) - problem, goals, HLD/LLD, vocabulary mapping, B-42, security, EARS acceptance.
- [PLAN.md](PLAN.md) - phases, dependency graph, gates and rollout.
- [TASKS.md](TASKS.md) - ordered plan 018-T01 to 018-T18 to V1 VERIFIED.
- [DATA-MODEL.md](DATA-MODEL.md) - usage record fields, projections, diagnostic files and retention.
- [RESEARCH.md](RESEARCH.md) - sources inspected, pinned references and decisions.
- [WIRING.yaml](WIRING.yaml) - components, edges, transactions and external boundaries.
- [contracts/](contracts/README.md) - API/event, errors/recovery and UI behavior.

## Dependencies

Feature 001 (usage write at settlement, B-10), Feature 002 (quota producers, Agent profile entry), Feature 011 (Settings shell, Runtime readiness, IPC), Feature 019 (secret scan, soak harness, release matrix, evidence bundle).

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [V1 release contract](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Telemetry, usage and resilience LLD](../../docs/V1-TELEMETRY-USAGE-RESILIENCE-LLD.md), [Observability](../../docs/OBSERVABILITY.md), [Security](../../docs/SECURITY.md), [API](../../docs/API.md), [Schemas](../../docs/SCHEMAS.md), [Runtime Lifecycle](../../docs/RUNTIME-LIFECYCLE.md), [UI screen contract](../../docs/V1-UI-SCREEN-CONTRACT.md), [Settings ledger](../../docs/V1-SETTINGS-CONTRACT-LEDGER.md), [Acceptance and E2E plan](../../docs/ACCEPTANCE-AND-E2E-PLAN.md), [Feature 001](../001-conversation-session-supervisor/SPEC.md).
