# 011 - Desktop Shell IPC Settings and Runtime Health

**Status:** DRAFT - review required; does not authorize implementation.
**Revision:** 0.2 | **Updated:** 2026-11-06
**Owners to approve:** Runtime, Operator IPC, Desktop UI, Product, Security/Trust.

## User outcome

The owner reaches the local Runtime over authenticated IPC and uses a Settings surface whose categories, scopes, saved values and action states are explicit. No Settings control looks operational unless its owner operation or native preference mechanism is actually qualified.

## V1 Settings surface

The category IA and per-tab behavior are defined in SPEC.md and docs/V1-UI-SCREEN-CONTRACT.md. The control-by-control authority, scope, operation class and failure behavior remain in docs/V1-SETTINGS-CONTRACT-LEDGER.md. The ledger is a target inventory, not a statement that all 187 controls are served.

The target includes General, Notifications, Workspaces, Agents, Connections & Tools, Local Models, Coworkers & Subagents, Runtime, Terminal, Browser & Computer, Memory & Knowledge, Security & Privacy, Storage & Backup, Usage & Monitoring, Advanced and About & Diagnostics. A category may be absent or read-only in the shipped UI until its owner operation is qualified.

## Current truth

The mounted SettingsView is a four-category partial surface: General, Workspaces, Agents and Runtime. Workspace selection/create/instruction revisions/archive, bounded agent catalogue/probe/binding actions and Runtime start/refresh/lifecycle observation are present. General appearance controls are inert component-local state; shortcut labels are display-only. WorkspaceRoot management, Workspace policy mutation and Runtime enrollment are not rendered by this component. The alternate SettingsPage in App.tsx is unmounted. Earlier nine-category and Browser/cache/TLS/Shell Engine/approval/Credential Store claims are stale; Workbench Browser/Terminal belong to Feature 008. See docs/SETTINGS-AND-SHELL-SURFACE-AUDIT.md and docs/IMPLEMENTATION-STATUS.md.

Usage projections and historical usage UI are not served. The telemetry LLD is normative target only. The new Settings specification does not authorize fake values or placeholder controls.

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