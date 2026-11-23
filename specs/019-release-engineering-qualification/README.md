# 019 - Release Engineering and V1 Qualification

- **Status:** IN_REVIEW - authored complete for owner review on 2026-11-09; not owner-accepted; authorizes no implementation; not a claim of implementation or qualification.
- **Revision:** 0.1 | **Updated:** 2026-11-09 | **Blockers:** B-43 (signing identity and update channel), B-44 (licensing/packaging decision), both PROPOSED, pending owner.
- **Historical implementation snapshot (2026-11-10):** CODE skeleton, nothing run on GitHub. This dated status is superseded by [CURRENT-RUN.md](../../CURRENT-RUN.md) and [IMPLEMENTATION-STATUS.md](../../docs/IMPLEMENTATION-STATUS.md).
- **2026-11-20 current slice:** T14/T15 have CODE implementations; T24 is IMPLEMENTING under USER-DELEGATED direction to cover the full adopted journey roster and enforce exact evidence-row matching. Matrix generation is not native-row qualification. Feature 019 remains IN_REVIEW; no package acceptance, release bundle or QUALIFIED journey is claimed.
- **Owners:** Release Engineering, Security, Desktop Shell, Runtime (Feature 011 interface), QA/Accessibility, Product.
- **Release matrix rows:** Windows 11/10 x64 + OpenCode v2.0.32 first; Linux x64 + OpenCode v2.0.32 second; WSL2 never counts.

## User outcome

The owner installs a signed LiteCowork build on Windows (then Linux), updates it only when the Runtime proves nothing is running, and reads a qualification matrix where every claimed cell links to CODE, SYSTEM and owner USER evidence for that exact row. V1 ships only after an explicit go/no-go that lists every package's VERIFIED evidence.

## Current truth

The release blockers remain substantial: CI, signing/updater, licensing/notices, native provider journeys, and per-row release evidence are incomplete. The matrix tools are now owned by this feature and are CODE; T24 corrects journey coverage, the active OpenCode HTTP/SSE route label and exact-row bundle validation. Windows and Linux release rows still require their respective native packaged journeys and owner USER evidence. No matrix generation on a development host qualifies a release.

## Package

- [SPEC.md](SPEC.md) - problem, goals, decisions B-43/B-44, HLD/LLD, security, EARS acceptance.
- [PLAN.md](PLAN.md) - phases, waves, dependency graph, rollout and gates.
- [TASKS.md](TASKS.md) - T01 to T24 including exact-row evidence validation and the V1 go/no-go.
- [DATA-MODEL.md](DATA-MODEL.md) - release rows, cells, bundles, update attempts and decision records.
- [RESEARCH.md](RESEARCH.md) - pinned sources, Tauri updater and signing references, alternatives.
- [WIRING.yaml](WIRING.yaml) - CI, release job, signing, updater, gate and matrix wiring.
- [contracts/](contracts/README.md) - API/command, errors/recovery and UI behavior.

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [V1 release contract](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Acceptance and E2E plan](../../docs/ACCEPTANCE-AND-E2E-PLAN.md), [Testing](../../docs/TESTING.md), [Security](../../docs/SECURITY.md), [Network security](../../docs/NETWORK-SECURITY.md), [Benchmarks](../../docs/BENCHMARKS.md), [Build tasks](../../docs/LITECOWORK-BUILD-TASKS.md), [Local Operator IPC](../../docs/LOCAL-OPERATOR-IPC.md), [Runtime lifecycle](../../docs/RUNTIME-LIFECYCLE.md), Feature [011](../011-desktop-shell-ipc-settings-runtime/README.md).
