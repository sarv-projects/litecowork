# 020 - Proactivity and Quiet Learning

- **Status:** IN_REVIEW - authored complete for owner review on 2026-11-09; not owner-accepted; authorizes no implementation; not a claim of implementation or qualification.
- **Revision:** 0.2 | **Updated:** 2026-11-20 | **Decision:** B-45 and D-01 remain ACCEPTED as proposed by the owner on 2026-11-10; the whole SPEC remains IN_REVIEW. The repository owner delegated implementation of the isolated deterministic gate repair (T03) and its F-04 module placement on 2026-11-20. This direction does not accept the package or authorize the other tasks.
- **Implementation state (2026-11-20):** T03 pure-gate repair is CODE-complete in the working tree with 34 domain-gate tests passing and 50 storage-core Skill tests passing. The package remains IN_REVIEW; D-01 and B-45 are accepted as proposed, but the whole SPEC is not accepted. Suggestion production, learning persistence/service/routes/UI and qualification remain absent. Migration v44 is an old allocation and must be rechecked against the current migration chain before use.
- **Owners:** Responsibility domain, Coworker, Storage, Operator/API, Desktop UI, Security/Trust, Product.
- **Release matrix rows:** Windows 11/10 x64 + OpenCode v2.0.32 first; Linux x64 + OpenCode v2.0.32 second.

## User outcome

The user sees a few explained ideas that never act on their own and respect quiet hours. A Coworker quietly remembers the user's explicit, low-risk private preferences with a subtle "Remembered" notice and Undo, sends anything riskier to a review list, and never learns from third-party or model content.

## Current truth

Suggestions, Goals and Needs You are SERVED with UI (`suggestions_operator.rs`, `goal_operator.rs`, `needs_you_operator.rs`, `suggestions_bridge.rs`, `src/suggestions/`), but nothing produces a Suggestion, so the Ideas page is empty in a real install. The learning gates are pure functions with no caller, storage, route or UI. T03 repairs their source-kind/channel gate and stable disposition tokens; it does not create an active learning pipeline or change which sources are eligible. Feature 009 defers automatic learning (B-24) and names this package as the pipeline owner.

## Package

- [SPEC.md](SPEC.md) - problem, goals, D-01 and B-45, HLD/LLD, security, EARS acceptance.
- [PLAN.md](PLAN.md) - phases, dependency graph, rollout and gates.
- [TASKS.md](TASKS.md) - T01 to T17.
- [DATA-MODEL.md](DATA-MODEL.md) - Suggestion (existing) and learning records, lifecycles, retention, migration.
- [RESEARCH.md](RESEARCH.md) - sources, pinned references, alternatives.
- [WIRING.yaml](WIRING.yaml) - producers, services, stores, bridges and UI.
- [contracts/](contracts/README.md) - API/events, errors/recovery and UI behavior.

## Canonical owners

[Responsibilities](../../docs/RESPONSIBILITIES.md), [Learning](../../docs/LEARNING.md), [ADR-0016](../../docs/adr/0016-coworker-goal-and-suggestion-authority-boundary.md), [ADR-0023](../../docs/adr/0023-host-guidance-is-not-capability-authority.md), [ADR-0024](../../docs/adr/0024-optional-chat-first-coworkers.md), [Coworker flows](../../docs/COWORKER-FLOWS.md), [API](../../docs/API.md), [Events](../../docs/EVENTS.md), [Benchmarks](../../docs/BENCHMARKS.md), Features [009](../009-coworkers-scoped-memory/README.md) and [010](../010-automations-recurring-host/README.md).
