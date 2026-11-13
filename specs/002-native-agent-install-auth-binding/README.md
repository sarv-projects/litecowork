# 002 - Native Agent Install, Auth and Binding

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Revision:** 1.0 | **Updated:** 2026-11-13
**Implementation state (2026-11-13):** CODE/SERVED is partial; there is no packaged native-provider journey or real sign-in qualification. T01, T03 and T04 are CODE-COMPLETE; T05 is PARTIAL with served OpenCode descriptor GET/refresh using the pre-existing migration-16 table, while option/capability projections are not served; T06, T07, T09, T10, T11, T12, T13, T14, T16, T17 and T18 are PARTIAL; T08 is BLOCKED (retryable) on an owner-approved cross-store replay/reconciliation contract, not on migration numbering, and remains safely fail-closed; T15 is DEFERRED by owner instruction; T19 is NOT STARTED. The auth launcher/opener are fail-closed; auth observations are in-memory and reset to UNKNOWN. The descriptor refresh budget is in-memory and resets on daemon restart. Do not treat provisionally reserved schema numbers as proof of a required or landed migration; see package TASKS and global migration allocation.
**Owners:** Agent Runtime, Security/Trust, Desktop UI, Product, Conversation Runtime. Accepted by the repository owner via [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md).

## User outcome

The owner discovers and inspects a native agent, establishes a module-qualified installation/profile and native authentication, reviews its capabilities, creates a disabled Workspace AgentBinding and enables it only after fresh descriptor and route readiness checks. The UI keeps every lifecycle axis independently observable.

## Current truth

The active checkout has 15 curated candidates plus read-only ACP metadata and bounded version/path observations; authentication remains UNKNOWN and readiness NOT_PROBED. Profile probes are limited to Codex and OpenCode: Codex observations do not imply Conversation dispatch, while OpenCode now produces a compatible `STARTABLE` offer in code only for exact 2.0.32, a complete probe and observed host stop. Descriptor GET/refresh is served for OpenCode and uses existing migration-16 storage; module-less profiles still fail closed. Auth-begin and native-config-open use authenticated ID-only Tauri commands but the daemon's production launcher/opener fail closed because no V1 module declares a qualified method or target. Credential submission returns 503 before SecretStore mutation; its descriptor source and durable replay receipt are missing. Binding enable enforces descriptor revision CAS only for modules declaring a descriptor; the legacy module-less path remains a gap. Install/update/repair, mounted model/effort selection, and Usage are incomplete. Feature 001 remains the authority for Conversation execution. Do not infer qualification from a catalogue row, model list, installed binary, profile probe or enabled binding. See the task-by-task evidence in TASKS.md and docs/IMPLEMENTATION-STATUS.md.

## V1 design

The catalogue, Agent profile sections, action preconditions and status projections are defined in SPEC.md and contracts/ui-behavior.md. The screenshot hierarchy is adopted without treating the old NeuralAgent repository or public discovery metadata as trusted install authority.

## Lifecycle, scope and dependencies

- Lifecycle: IMPLEMENTING (target decisions were owner-accepted and locked on 2026-11-09; implementation started with 002-T01). VERIFIED remains unavailable until the final TASKS.md evidence and owner sign-off are recorded.
- Release target: V1 release matrix row 1 (Windows x64 + OpenCode v2.0.32), row 2 (Linux x64). Claude Code CLI is the second AgentModule (B-11).
- Screen ownership (A-35): the Settings -> Agents screen is owned by [Feature 011](../011-desktop-shell-ipc-settings-runtime/SPEC.md). Feature 002 owns only the install/auth/descriptor/binding operations that screen invokes.
- Consumers: [Feature 001](../001-conversation-session-supervisor/SPEC.md) needs a creatable OpenCode binding (P1 prerequisite); [Feature 004](../004-agent-relay/SPEC.md) needs the second module.
- By owner approval on 2026-11-09 (see [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md)), this package folds proposed tasks P-11 (002-T17, define the native-agent adapter as an ACP client, register [A-54](../../docs/CONTRADICTION-AND-ALIGNMENT-REGISTER.md)) and P-12 (002-T18, Agent Matchmaker). Source: [docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md](../../docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md) section 8. The ACP SDK pin is recorded in [RESEARCH.md](RESEARCH.md).

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