# 017 - Data Portability, Backup and Restore

- **Status:** IN_REVIEW - B-41 was ACCEPTED as proposed on 2026-11-10 (A-62); whole-SPEC owner acceptance remains pending. USER-DELEGATED A-117/A-118 authorize storage-only T03 implementation, not owner approval or qualification.
- **Revision:** 0.2 | **Updated:** 2026-11-23 | **Delivery wave:** Wave 6 (gate G) | **E2E:** E2E-12 | **Master plan:** P11
- **Implementation state (2026-11-23):** T03 has scoped CODE for exhaustive Workspace table classification, one-Workspace row/blob selection, local/authority exclusion, unfinished upload exclusion and canary scanning. Related row pruning uses one deferred-FK SQLite transaction and revalidates the committed copy. The CLI remains the only user-reachable backup surface. No Operator route, Tauri bridge, Settings UI, portable import, packaged journey or qualification is established.
- **Owners:** Storage, Operator/API, Desktop UI, Security/Trust, Runtime Lifecycle and Product.

## User outcome

From Settings > Storage & Backup the owner can create and verify an encrypted Workspace backup, export a portable archive, import it into another installation, restore an installation offline after a safe Runtime stop, and confirm that deletions, tombstones, Evidence and provenance survived, with no secret ever carried.

## Current truth

`litecoworkd backup create|verify|restore|upgrade-check` exists as a CLI (`apps/litecoworkd/src/backup_operator.rs`) over the encrypted `LocalBackupService` (`crates/storage-sqlite/src/backup.rs`). T03 CODE now filters the snapshot to one Workspace and validates the complete 142-table classification before publishing. It excludes local/authority records and unfinished upload state, prunes related rows in one immediate transaction with deferred FK validation, derives blob candidates from the same copied database state before removing the local registry, and checks synthetic credential/locator/passphrase canaries across staged and decrypted output, including a foreign Conversation and local native-session binding. This is storage evidence only: the four backup operations in OpenAPI remain target-only, there is no Tauri command or Settings category, and the packaged desktop journey is absent. Portable re-encryption, staging crash cleanup, Windows owner-only DACLs, restore receipt time/new Runtime corrections, operation progress/cancellation/single-flight and release qualification remain incomplete. See [SPEC.md](SPEC.md) section 4, [TASKS.md](TASKS.md) T03/T04 and [CURRENT-RUN.md](../../CURRENT-RUN.md).

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
