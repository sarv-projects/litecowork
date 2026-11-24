# Plan - Desktop Shell IPC Settings and Runtime Health

**Feature:** 011 | **Status:** ACCEPTED/LOCKED - owner-accepted 2026-11-09; authorizes implementation; not a claim of implementation or qualification.
**Spec revision:** 1.0 | **Delivery wave:** 0 (foundation), with the stop/update gate closing after Features 001 and 005 settlement.

## Phases

1. SPEC section 5 blockers B-28, B-29, B-33 and A-35 are ACCEPTED (2026-11-09).
2. Promote the accepted gap interfaces (renameWorkspace, stop preview payload, update quiescence gate, power assertion, local preference store) in the canonical owner docs and `docs/schemas/` in one change.
3. Wave 0 foundation: Windows named-pipe DACL qualification, Linux socket qualification, IPC bounds, Runtime health honesty, inert-control removal, local preferences, keyboard/focus baseline.
4. Settings owner work: Workspace rename/archive, Settings -> Agents screen over Feature 002 operations, Notifications link to Feature 010, keep-awake.
5. Stop/update gate after Feature 001 Stop/quiescence and Feature 005 Effect/lease settlement.
6. Failure/recovery, cross-layer traces, packaged qualification on Windows x64 then Linux x64, documentation sync, VERIFIED transition.

### User-delegated shell continuation (A-123; 011-T19)

Continue the existing shell task with New Chat, Recent Chats, Projects, Needs You, More and
footer Settings. Projects is a read-only directory over existing Workspace records; it must
not add a Project identity, membership schema, owner route or authorization boundary. Move
less-frequent domains under More, preserve every mounted route, the existing dirty-Library
guard and keyboard/focus behavior. Recent Chats remains Conversation-only until cross-owner
ordering, pagination, partial-source freshness and recovery are contracted. A Needs You count
is omitted unless an owner projection provides an exact, non-duplicated count. The active
four-destination V1 contract remains formally unchanged; the task is authorized by the
product owner's implementation delegation, not by a new package acceptance. No migration is
required.

At widths up to 900px the sidebar is a modal drawer. Since the Conversation route does not
render the standard page topbar, its header must expose a Menu trigger wired to the same drawer
state. The shared trigger ref moves across routes so closing the drawer returns focus to the
currently mounted screen; an off-canvas sidebar is not a reachable navigation path by itself.

The ordered task list is [TASKS.md](TASKS.md).

## Dependency DAG

- T01 (contract promotion) blocks tasks that add or change an interface; T02-T07 and T11 only harden existing code and may start in parallel.
- T13 and T14 wait on Feature 001 and Feature 005 settlement.
- External side effects never run inside the transactions declared in WIRING.yaml.
- Feature 001 remains a prerequisite wherever this slice needs a committed Conversation turn.

## Owners and file scope

Owners: Runtime, Operator IPC, Desktop UI, Product.
Allowed implementation paths are listed per task. The active source tree is in apps/ and crates/. Changes remain limited to the task-owned paths and require package acceptance, a clean selected worktree, and source/contract/UI/test reconciliation.

## Migration, rollout, rollback

Workspace rename needs no migration: the Workspace record already has `name` and `version` (`docs/schemas/sqlite-v1.sql`, OpenAPI `Workspace`). Local display preferences use the existing validated device-local localStorage snapshot, not SQLite or domain policy storage. A later schema change uses the next immutable SQLite migration. Rollback is revert to the previous committed revision and refuse new admission that needs the new field.

## Risks

- Mounting the stop route before settlement is proven.
- Counting WSL2 runs as Windows or Linux evidence.
- Treating archive CODE as current qualification.
- Collapsing distinct states into one UI boolean.

## Verification gates

- `python3 .specify/scripts/validate_feature_package.py`
- Owner acceptance recorded 2026-11-09
- TASKS.md commands and denial, replay, restart and cancel cases
- Desktop stop/update qualification gate journey on each row

## DeepSeek Harness proposals (PROPOSED, owner review required)

PROPOSED only; see the matching section in [TASKS.md](TASKS.md). Nothing here changes the accepted design of this package. Owner review is required first.

- **P-31 QuiescenceReport (one "interruptible work" predicate; unknown means busy; lock then learn; inspect once)** (owns the proposed row). Class **ADAPT**. Design impact if accepted: T12, T13, T14 of package 011; T11 of package 019; T10 of package 010. Deferral: Daemon half (`RuntimeLifecycleStore`, served `previewLocalRuntimeStop`) can be built and tested on Linux; the packaged quit path needs the Windows laptop. Not started. Upstream: [src/host-process.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/host-process.ts), [src/quit-confirmation.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/quit-confirmation.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 3 and 14 (row P-31).
- **P-32 Per-request deadlines with defined meaning-on-timeout for destructive shell-to-daemon calls** (owns the proposed row). Class **ADAPT**. Design impact if accepted: T04 and T13 of package 011; T07 of package 019. Deferral: Not started. Needs the operator IPC and Tauri bridge decisions in package 011 first. Upstream: [src/host-process.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/host-process.ts), [src/welcome-window.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/welcome-window.ts), [src/update-dialog.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/update-dialog.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 2, 6 and 14 (row P-32).
- **P-37 Golden files for user-visible dialog copy and golden scenario lists for installer tests** (owns the proposed row). Class **ADAPT**. Design impact if accepted: T19 of package 011; T16 and T08 of package 019. Deferral: UI golden tests can run on Linux; installer goldens wait for the Windows laptop. Upstream: [tests/expected/](https://github.com/deepseek-ai/deepseek-harness/tree/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/tests/expected), [tests/expected/windows-installer.json](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/tests/expected/windows-installer.json), [tests/expected/windows-uninstall.json](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/tests/expected/windows-uninstall.json), [src/locale.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/locale.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Sections 12 and 14 (row P-37).
- **P-46 IME-safe shortcut dispatch rules (no match during composition, dead keys or AltGr; reset on blur; balanced keyups)** (owns the proposed row). Class **ADAPT (rules only)**. Design impact if accepted: T11 of package 011. Deferral: UI tests on Linux; not started. Upstream: [src/keyboard.ts](https://github.com/deepseek-ai/deepseek-harness/blob/fcc0ad462172522aaf5c93a7b2e01f6b9f0b0159/apps/desktop/src/keyboard.ts). Study: [desktop study](../../docs/DEEPSEEK-HARNESS-DESKTOP-DEEP-DIVE-2026-11.md) Section 14 (row P-46).

