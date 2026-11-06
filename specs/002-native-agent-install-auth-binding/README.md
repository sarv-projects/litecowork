# 002 - Native Agent Install, Auth and Binding

**Status:** DRAFT - review required; does not authorize implementation.
**Revision:** 0.3 | **Updated:** 2026-11-06
**Owners to approve:** Agent Runtime, Security/Trust, Desktop UI, Product, Conversation Runtime.

## User outcome

The owner discovers and inspects a native agent, establishes a module-qualified installation/profile and native authentication, reviews its capabilities, creates a disabled Workspace AgentBinding and enables it only after fresh descriptor and route readiness checks. The UI keeps every lifecycle axis independently observable.

## Current truth

The active checkout has 15 curated candidates plus read-only ACP metadata and bounded version/path observations; auth remains UNKNOWN and readiness NOT_PROBED. Profile probes are limited to Codex and OpenCode: Codex observations do not imply Conversation dispatch, while OpenCode's current profile offer is incompatible. Install/update/repair and native sign-in are absent; descriptor/credential routes return 503; binding enable does not enforce the fresh AC-004 descriptor gate. Model/effort selection and Usage are not qualified. Feature 001 remains the authority for Conversation execution. Do not infer qualification from a catalogue row, model list, installed binary, profile probe or enabled binding.

## V1 design

The catalogue, Agent profile sections, action preconditions and status projections are defined in SPEC.md and contracts/ui-behavior.md. The screenshot hierarchy is adopted without treating the old NeuralAgent repository or public discovery metadata as trusted install authority.

## Package

- [SPEC.md](SPEC.md)
- [PLAN.md](PLAN.md)
- [TASKS.md](TASKS.md)
- [DATA-MODEL.md](DATA-MODEL.md)
- [RESEARCH.md](RESEARCH.md)
- [WIRING.yaml](WIRING.yaml)
- [contracts/](contracts/README.md)

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [Product](../../docs/PRODUCT.md), [V1 release](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Master plan](../../docs/IMPLEMENTATION-MASTER-PLAN.md), [Agent Discovery LLD](../../docs/V1-AGENT-DISCOVERY-AND-CATALOG-LLD.md), [Telemetry LLD](../../docs/V1-TELEMETRY-USAGE-RESILIENCE-LLD.md), [Settings control ledger](../../docs/V1-SETTINGS-CONTRACT-LEDGER.md).