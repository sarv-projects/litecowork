# 011 - Desktop Shell IPC Settings and Runtime Health

**Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Revision:** 1.0 | **Updated:** 2026-11-09 | **Owner acceptance:** repository owner, 2026-11-09, via [OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md)
**Implementation evidence:** the 2026-11-10 task list above is a historical checkpoint, not current status. For this checkout, read the per-task `TASKS.md` and current evidence in [`docs/IMPLEMENTATION-STATUS.md`](../../docs/IMPLEMENTATION-STATUS.md) and [`CURRENT-RUN.md`](../../CURRENT-RUN.md). As of the 2026-11-23 recheck, T19 remains PARTIAL; the compact-navigation/Workspace-backed Projects implementation is user-directed under A-123 and does not amend package acceptance. T23 remains the package verifier. Native packaged qualification is still separate.
**Component owners:** Runtime, Operator IPC, Desktop UI, Product, Security/Trust.

## User outcome

The owner reaches the local Runtime over authenticated IPC and uses a Settings surface whose categories, scopes, saved values and action states are explicit. No Settings control looks operational unless its owner operation or native preference mechanism is actually qualified.

## V1 Settings surface

The category IA and per-tab behavior are defined in SPEC.md and docs/V1-UI-SCREEN-CONTRACT.md. The control-by-control authority, scope, operation class and failure behavior remain in docs/V1-SETTINGS-CONTRACT-LEDGER.md. The ledger is a target inventory, not a statement that all 187 controls are served.

The target includes General, Notifications, Workspaces, Agents, Connections & Tools, Local Models, Coworkers & Subagents, Runtime, Terminal, Browser & Computer, Memory & Knowledge, Security & Privacy, Storage & Backup, Usage & Monitoring, Advanced and About & Diagnostics. A category may be absent or read-only in the shipped UI until its owner operation is qualified.

## Current truth

The mounted SettingsView is a four-category partial surface: General, Workspaces, Agents and Runtime. Workspace selection/create/instruction revisions/archive, bounded agent catalogue/probe/binding actions and Runtime start/refresh/lifecycle observation are present. General appearance controls are inert component-local state; shortcut labels are display-only. WorkspaceRoot management, Workspace policy mutation and Runtime enrollment are not rendered by this component. The alternate SettingsPage in App.tsx is unmounted. Earlier nine-category and Browser/cache/TLS/Shell Engine/approval/Credential Store claims are stale; Workbench Browser/Terminal belong to Feature 008. See docs/SETTINGS-AND-SHELL-SURFACE-AUDIT.md and docs/IMPLEMENTATION-STATUS.md.

Accepted decisions (2026-11-09): B-28 Windows 11/10 x64 named-pipe DACL profile first (OpenCode v2.0.32 the first route), Linux x64 0600/0700 socket second, WSL2 never counts; B-29/B-33 only served-owner Settings controls are interactive and inert controls are removed; A-35 this package owns the Settings -> Agents screen and links to Feature 010 for quiet hours. Revision 1.0 adds the authenticated Operator stop and update/quiescence gate, keep-awake, Workspace rename/archive in Settings and an app-wide keyboard/focus baseline. The authenticated stop route stays unmounted (`apps/litecoworkd/src/runtime.rs`) until settlement is proven. The V1 plan is in [TASKS.md](TASKS.md). Usage dashboards are owned by Feature 018; Backup by Feature 017.

Usage projections and historical usage UI are not served. The telemetry LLD is normative target only. The new Settings specification does not authorize fake values or placeholder controls.

By owner approval on 2026-11-09 (see [docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md](../../docs/OWNER-ACCEPTANCE-REVIEW-PACKET.md)), this package folds proposed tasks P-05 (011-T19, shell reachability and truthfulness), P-07 with P-19 (011-T20, decompose App.tsx and the Rust god files behind existing boundaries, no behavior change), P-08 (011-T21, merge SettingsPage into SettingsView with precedence notes) and P-16 (011-T22, persistent Workspace Dashboard with pinnable widgets). Sources: [docs/UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md](../../docs/UI-RENDERING-SETTINGS-ARCHITECTURE-AUDIT-2026-11.md) section 9 and [docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md](../../docs/REFERENCE-SYNTHESIS-AND-V1-FEATURE-ALIGNMENT-2026-11.md) section 8.

## Package

- [SPEC.md](SPEC.md)
- [PLAN.md](PLAN.md)
- [TASKS.md](TASKS.md)
- [DATA-MODEL.md](DATA-MODEL.md)
- [RESEARCH.md](RESEARCH.md)
- [WIRING.yaml](WIRING.yaml)
- [contracts/](contracts/README.md)

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [Product](../../docs/PRODUCT.md), [V1 release](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Master plan](../../docs/IMPLEMENTATION-MASTER-PLAN.md), [Settings control ledger](../../docs/V1-SETTINGS-CONTRACT-LEDGER.md), [UI screen contract](../../docs/V1-UI-SCREEN-CONTRACT.md), [Telemetry LLD](../../docs/V1-TELEMETRY-USAGE-RESILIENCE-LLD.md), [Agent Discovery LLD](../../docs/V1-AGENT-DISCOVERY-AND-CATALOG-LLD.md).
