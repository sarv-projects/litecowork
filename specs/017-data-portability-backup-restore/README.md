# 017 - Data Portability, Backup and Restore

- **Status:** IN_REVIEW - authored 2026-11-09 after the owner acceptance packet; B-41 was ACCEPTED as proposed on 2026-11-10 (A-62) and whole-SPEC acceptance is pending; does not authorize implementation and is not a claim of implementation or qualification.
- **Revision:** 0.1 | **Updated:** 2026-11-09 | **Delivery wave:** Wave 6 (gate G) | **E2E:** E2E-12 | **Master plan:** P11
- **Implementation state (2026-11-10):** Not implemented beyond a CLI-only backup operator; migration v46 allocated.
- **Owners:** Storage, Operator/API, Desktop UI, Security/Trust, Runtime Lifecycle and Product.

## User outcome

From Settings > Storage & Backup the owner can create and verify an encrypted Workspace backup, export a portable archive, import it into another installation, restore an installation offline after a safe Runtime stop, and confirm that deletions, tombstones, Evidence and provenance survived, with no secret ever carried.

## Current truth

`litecoworkd backup create|verify|restore|upgrade-check` exists as a CLI (`apps/litecoworkd/src/backup_operator.rs`) over a real encrypted container implementation (`crates/storage-sqlite/src/backup.rs`, `LocalBackupService`) with 11 storage tests. Nothing is reachable from the desktop: the four backup operations in OpenAPI are target-only, there is no Tauri command and no Settings category. Source reading found that the snapshot is not filtered to the requested Workspace, portable restore on a new machine is impossible because blob keys stay in the source OS keystore, a crash can leave a plaintext staging snapshot, Windows staging has no owner-only DACL, and the restore receipt reports the wrong time. See [SPEC.md](SPEC.md) section 4.

## Package

- [SPEC.md](SPEC.md) - problem, goals, HLD/LLD, B-41, security, errors, EARS acceptance.
- [PLAN.md](PLAN.md) - phases, dependency graph, gates and rollout.
- [TASKS.md](TASKS.md) - ordered plan 017-T01 to 017-T18 to V1 VERIFIED.
- [DATA-MODEL.md](DATA-MODEL.md) - manifests, operations, archive classes, lifecycle and retention.
- [RESEARCH.md](RESEARCH.md) - sources inspected, alternatives and decisions.
- [WIRING.yaml](WIRING.yaml) - components, edges, transactions and external boundaries.
- [contracts/](contracts/README.md) - API/event, errors/recovery and UI behavior.

## Dependencies

Feature 011 (Settings shell, stop/quiescence gate, IPC), Feature 003 (ContextDocument purge), Feature 014 (Conversation delete tombstones), Feature 019 (release matrix, evidence bundle, updater pre-flight), and data owners 001, 007, 009, 010 whose records the archive carries.

## Canonical owners

[Architecture](../../ARCHITECTURE.md), [V1 release contract](../../docs/V1-PRODUCT-RELEASE-CONTRACT.md), [Storage](../../docs/STORAGE.md), [Security](../../docs/SECURITY.md), [Failure Recovery](../../docs/FAILURE-RECOVERY.md), [API](../../docs/API.md), [Events](../../docs/EVENTS.md), [Local Operator IPC](../../docs/LOCAL-OPERATOR-IPC.md), [Runtime Lifecycle](../../docs/RUNTIME-LIFECYCLE.md), [UI screen contract](../../docs/V1-UI-SCREEN-CONTRACT.md), [Settings ledger](../../docs/V1-SETTINGS-CONTRACT-LEDGER.md), [Acceptance and E2E plan](../../docs/ACCEPTANCE-AND-E2E-PLAN.md).
