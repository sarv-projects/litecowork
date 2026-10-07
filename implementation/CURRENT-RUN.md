# Current run and agent handoff

Update this file when the active story, implementation state, or handoff changes. It is
operational context; architecture and owning domain contracts remain authoritative.

## Active continuation — SP04 handover PoC

The owner redirected the active work to the Codex (`gpt-6-luna`, medium) → OpenCode
handover feasibility question. This is **not a coding-speed benchmark**. The current local
prototype materializes manifest-listed checkpoint files into a digest-addressed, read-only
snapshot before sender settlement and re-verifies that snapshot at receiver admission. The
Linux bubblewrap supervisor passes manifest-checked, sealed `memfd` file contents as
read-only receiver mounts while leaving the receiver project directory writable. Forty-two
focused PoC tests pass. Mutating the original
sender workspace after settlement leaves the pinned snapshot unchanged; tampering with the
stored snapshot blocks admission. An unchanged snapshot is admitted after ledger restart at
lease epoch 2. An integrated Linux fixture now runs sender stop/quiescence → snapshot → ledger
settlement → replacement admission → receiver mount; the receiver reads the pinned bytes,
cannot write the mounted snapshot, updates the working project file, and writes in its project
directory. Host changes after sandbox launch do not change the receiver's sealed bytes, and a
pre-launch digest mismatch prevents worker start. Replacement state distinguishes `ADMITTED`,
`RUNNING`, and `START_FAILED`; a failed launch releases the Task's active slot while consuming
its lease epoch, and retry starts at a higher epoch. Effect reconciliation and lease release
remain mocked, and this is not product Task/Attempt integration. Packaging is capped at 4,096
files and 512 MiB total; oversized checkpoints fail before worker launch.

A Linux restart test crashes the Runtime after launching the contained heartbeat writer but
before committing `RUNNING`. Recovery blocks while the launcher identity is live; after it exits,
the admission becomes `START_FAILED`, and retry uses lease epoch 3. A second test crashes the
Runtime after `RUNNING` is committed. The ledger pins both the Runtime launcher identity and the
contained bubblewrap process identity; recovery waits for both to be proven gone, marks the old
Attempt `ABANDONED`, records mocked Effect/lease reconciliation, and retries at epoch 3. Both
tests require a stable heartbeat quiet interval. PID reuse and old-boot identities are rejected,
unreadable identity data fails closed, and an unresolved Attempt still blocks duplicate writers.
The two Runtime-crash recovery tests were each repeated five additional times; all 10 repeat runs
passed. This is repeatability evidence for the Linux fixture transitions only.
This remains a Linux PoC; it is not integrated with product RuntimeLifecycle, TaskService, real
lease fencing, or Effect reconciliation.

The 2026-10-07 review pass is recorded in
[`reviews/SP04-HANDOVER-REVIEW.md`](reviews/SP04-HANDOVER-REVIEW.md). HO-01 through HO-04
scratch diffs were unavailable; HO-05 was source-reviewed and corrected. This host is WSL2,
so native Linux/macOS/Windows containment remains unqualified. Product Task/Attempt/Lease/
Effect switching is not implemented; see the dependency sequence in the review packet. Do not
report switch speed as a pass criterion. See [`spikes/SP04.md`](spikes/SP04.md).

## Implementation backlog status (separate from the active SP04 PoC)

**E01-S02 — Transactional persistence** (`IN_PROGRESS`, owner review pending).

E01-S01 remains `IN_REVIEW`: owner clean-clone acceptance on the reference machine and
hosted GitHub Actions are still pending. This S02 work continues at the owner's direction;
it does not waive the dependency or claim either story accepted.

## Repository state

- Repository: `litecowork`
- Branch: `main`
- Current HEAD: `0ffa5f2 docs: avoid stale branch-count handoff`.
- The branch contains local commits ahead of `origin/main`; no push was performed.
- No push is authorized or performed in this run.
- V1 delivery order remains desktop/local, cloud continuation, then remote Runtime.
- Coding agents implement and report evidence; the owner reviews and accepts product
  behavior. Local tests do not establish hosted CI, owner-machine acceptance, provider
  qualification, or production readiness.

## Latest verification — 2026-10-07

- `python3 -m pytest -q implementation/spikes/agent_switching_poc/tests`: **42 passed**.
- HO-05 disposable fixture `python3 -m unittest -q`: **20 passed** after direct source review
  and regression fixes for integer conversion limits and not-before rounding.
- The two Runtime-crash recovery tests were each repeated five additional times: **10/10 passed**.
- `bash scripts/check.sh`: **passed**, including Rust workspace tests (25 total), architecture
  validation, 64-document/4,835-row coverage validation, and implementation-plan validation
  (55 stories).
- `git diff --check`: passed. No commit or push was made.

## Story status — 2026-10-07

| Area | Status | Evidence / remaining gate |
|---|---|---|
| E01-S01 toolchain and CLI foundation | `IN_REVIEW` | Local gate and recorded clean-clone run pass. Owner reference-machine acceptance and hosted GitHub Actions remain pending. |
| E01-S02 Workspace persistence | `IN_PROGRESS` | Workspace SQLite/blob slice and 19 storage tests are implemented; full local check passes. Owner review, disk-full, longer supported-hardware capacity/memory/shutdown checks, production key service/key rotation, cryptographic review, and daemon integration remain open. |
| Runtime lifecycle, authenticated Operator API, desktop | `NOT_IMPLEMENTED` | No operational daemon, Operator transport, or Tauri shell exists yet. |
| Task execution and safe agent switching | `NOT_IMPLEMENTED` | No durable Task/Attempt runtime, agent adapter, handoff coordinator, fencing path, or end-to-end switching test exists yet. |
| Handover PoC | `IN_PROGRESS — five disposable handovers reached task acceptance, with two independent-review corrections in HO-05; Linux fixture covers quiescence, sealed checkpoints, and recovery before and after RUNNING` | Cases 1–3: sequential Codex GPT-6 Luna/medium → OpenCode handovers; combined suites passed 9, 6, and 10 tests. Case 4: Codex partially implemented `normalize_slug` with `is_valid_slug` unfinished; native App Server turn interruption returned `interrupted` but the command kept writing for at least 20 seconds. After sender-host shutdown and a verified quiet interval, OpenCode Ling 3.1 Flash Free completed the task; all 6 tests passed and were independently rerun. MiMo free returned HTTP 429 for cases 2 and 4; Ling was used as fallback. The 42-test disposable suite materializes/revalidates snapshots, seals verified file bytes into memfds, rejects changed snapshots before launch, exercises sender quiescence and ledger settlement, distinguishes admitted/running/abandoned/start-failed replacement states, recovers Runtime crashes before and after RUNNING only after process identities are proven gone, fails closed on missing/unreadable identities or unresolved Effect/lease gates, retries at a higher epoch, tests Linux receiver mounting with a writable project directory, and confirms fail-closed duplicate prevention after ledger restart. It does not establish product Task/Attempt switching: Effect/lease reconciliation are mocked, product Runtime recovery is unimplemented, and other desktop OS containment remains unqualified. These cases establish preliminary checkpoint-continuation feasibility, not broad reliability. HO-01 through HO-04 source-level review is limited because their scratch trees are gone; HO-05 adds a sequential handover with independent HTTP grammar and input-boundary corrections; see [`spikes/SP04.md`](spikes/SP04.md). |

Five disposable coding handovers show the selected agents continued from bounded packets and
project state across three completed-subtask checkpoints, one partial implementation with
failing tests, and a fifth digest-pinned retry-policy checkpoint. The fourth receiver restated
the task, completed work, remaining requirements, and acceptance criteria before editing; its
manifest digest was checked, and its combined six-test suite passed independently. The fifth
receiver also restated its handoff correctly and completed the task; independent review found
and corrected one HTTP grammar edge. In that live case, Codex `turn/completed(interrupted)` did
not stop the spawned sandbox writer: its heartbeat advanced for 20.12 seconds while the App
Server remained alive. The sender host was stopped, then the heartbeat remained unchanged for
three seconds before OpenCode admission. Therefore an interrupted turn status is not a safe
switch fence; sender-host/environment shutdown and observed quiescence are required. This does
not establish broad reliability, product Task/Attempt persistence, actual lease fencing, Effect
reconciliation, provider-independent cancellation, or cross-platform containment. See
[`spikes/SP04.md`](spikes/SP04.md) for evidence and remaining gates. The attempt to run Codex
inside the fixture's read-only-root sandbox failed during native App Server initialization
before task work; it remains an adapter/Environment boundary finding, not a handover case.

## Completed implementation scope

- Added Rust `storage-core` ports for `StateStore`, `EventStore`, `BlobStore`, and their
  Workspace persistence composition.
- Added `domain-workspace::WorkspaceService` create and replication-policy update commands
  with expected-version checks and closed, versioned Workspace events.
- Added `storage-sqlite`: bounded writer actor; FK/WAL/FULL durability configuration;
  canonical v1 DDL migration; source checksum and resulting-schema fingerprint; bounded
  busy timeout; immediate transactions; atomic Workspace projection/event/origin-sequence
  commits; replay from immutable state blobs; and error mapping.
- Added encrypted content-addressed `FileBlobStore` with Workspace/purpose-scoped paths,
  XChaCha20-Poly1305 authenticated data, versioned injected keys, digest/size validation,
  atomic no-clobber writes, fsync, and fail-closed permissions/key errors. Tests use a
  fixed test-only key. There is no production key provider.
- Added adversarial tests for JCS ordering/number formatting, unsafe integer rejection,
  migration receipts/rollback, schema drift, concurrent stale writes, transaction rollback,
  busy timeout, replay, corrupt/wrong-scope blobs, unavailable keys, broad directory
  permissions, private SQLite state-directory/file permissions, symlink database paths,
  and failed BlobStore commits.
- Updated storage/event/schema docs, the E01-S02 story/backlog, research references and
  generated implementation coverage.

## Active SP02 qualification update

- Separate SQLx 0.9.0 and rusqlite 0.40.2 executables use the same `WorkspaceService`,
  production encrypted `FileBlobStore`, eight concurrent producers, 32-slot bounded writer,
  WAL/FULL settings, and host SQLite 3.45.1. SQLx is configured with `sqlite-unbundled`
  alone; the SQLx `sqlite` convenience feature also enables bundled SQLite. Product
  `storage-sqlite` remains bundled by default; only the rusqlite spike disables that default.
- Both executables close/reopen the database, verify the Workspace projection, decrypt
  aggregate-state blobs, replay events, and compare the replayed state. The rusqlite side
  uses the product adapter. SQLx is a prototype and lacks the product migration receipts,
  schema-drift checks, full path hardening/error mapping, and root-scope reads.
- Three sequential optimized 200-update runs per driver were alternated on Ubuntu 24.04.4
  LTS under WSL2, x86_64, Linux 6.18.40.1, Rust/Cargo 1.98.1, system SQLite 3.45.1.
  Median throughput was 327.4 updates/s for rusqlite and 322.9 for SQLx; observed ranges
  overlap, so this experiment does not select a driver. Exact samples and method are in
  `implementation/spikes/SP02.md`.
- `storage-sqlite` retains bundled SQLite and rusqlite's default cache/wasm-compatible
  features for production. The benchmark disables only SQLite bundling while explicitly
  preserving those rusqlite defaults. The storage runtime test wording now applies to both
  bundled and system SQLite builds.
- Added a rusqlite product-adapter mixed workload: 40 Workspace writers, four readers, a
  32-slot writer queue, and 1,200 updates per measured run. Readers check monotonic,
  policy-consistent snapshots; close/reopen verifies replay for all 40 Workspaces. Three
  sequential runs passed integrity checks but showed highly variable elapsed and tail
  latency. Exact queue occupancy is not exposed; longer capacity qualification remains open.
- Added process-local `SqliteWriterMetricsSnapshot` observations for outstanding command
  submissions and bounded-channel send wait. A held external writer lock test confirms
  blocked callers raise the high-water mark and settle without a partial Workspace write.
  This is runtime telemetry only; it is not durable state, exact queue occupancy, or an
  Operator metrics export.

## Verification status

Focused checks passed during this run:

- `uv run --locked python -m unittest discover -s implementation/spikes/agent_switching_poc/tests -v`:
  42 PoC tests passed, including Linux PID-namespace containment of a `setsid()` descendant,
  packet/checkpoint integrity rejection, malformed handoff, crash recovery, restart before
  replacement admission, immutable snapshot materialization, source-workspace isolation,
  tampered-snapshot admission rejection, sealed-byte handling under host mutation, launch-time
  digest rejection, size bounds, read-only receiver mounts with writable project workspaces,
  replacement startup failure cleanup, Runtime crash during worker launch, verified owner
  identity recovery before and after the replacement reaches `RUNNING`, PID-reuse/boot
  fencing, fail-closed missing-worker-identity handling, duplicate prevention, and same-lead
  retry at a fenced higher lease epoch.
- SP04-HO-04 in the disposable receiver fixture: `python3 -m unittest` passed all six tests
  in OpenCode and in an independent rerun; independent boundary checks covered six accepted
  values, ten rejected values, four non-string `TypeError` cases, and the sender's
  `normalize_slug` example. The sender heartbeat remained at 198 for three seconds after
  receiver completion. This correctness evidence is not a speed measurement.
- The disposable Codex → OpenCode coding task's combined suite: 9 tests passed after both
  agents completed; source and tests remain only under `/tmp/litecowork-sp04-coding-jsvc7ifu`.
- `cargo test --no-default-features --features sqlite-rusqlite-defaults -p storage-sqlite --offline`:
  19 passed using system SQLite while retaining rusqlite's non-bundle default features.
- `cargo test --manifest-path implementation/spikes/sp02-driver-compare/Cargo.toml --offline`:
  6 passed, including the encrypted Workspace restart/replay run and production blob tests.
- `cargo test --manifest-path implementation/spikes/sp02-driver-compare/rusqlite/Cargo.toml --offline`:
  2 passed using the product adapter, including mixed-load snapshot/replay correctness.
- Three release samples per driver completed; every run reported SQLite 3.45.1, eight
  replayed Workspaces and `replay_matches=true`.

The full `scripts/check.sh` passed with exit code 0 after refreshing generated coverage. It
ran Rust formatting, Clippy, build and 25 workspace Rust tests; six repository Python tests;
architecture validation; plan and coverage validation (64 documents, 870 sections, 4,827
traceability rows); and `git diff --check`. Locked SQLx and rusqlite spike test commands also
passed after the feature correction. Default production bundled SQLite remains SQLite 3.53.2
as previously recorded. Owner-machine acceptance and hosted GitHub Actions remain pending.

Exact review instructions and remaining product limitations are in
[`reviews/E01-S02.md`](reviews/E01-S02.md) and [`spikes/SP02.md`](spikes/SP02.md). SP02 and
E01-S02 remain partial/in progress. The mixed-load correctness and initial pressure sample
is complete, but longer supported-hardware capacity qualification, memory,
cancellation/shutdown, disk-full injection, key-service qualification, key rotation and
cryptographic review are still open.
The storage library is not yet wired into
`litecoworkd`, the desktop, Operator transport, or a real external provider.

## Next actions

1. Owner reviews the consolidated SP04 handover packet and its test evidence. HO-01 through
   HO-04 diffs were unavailable; HO-05 records the direct source review. The fourth case
   establishes a critical safety rule: a native Codex interrupted status did not stop its active
   sandbox command; replacement admission waited for sender-host shutdown and observed writer
   quiescence. HO-05 records independent HTTP grammar and input-boundary corrections.
   Do not optimize or report switch speed as an acceptance criterion.
2. Owner reviews the new post-`RUNNING` Runtime-crash case. The Linux PoC now verifies both
   process identities and heartbeat quiescence; Task/Lease/Effect recovery is still not
   integrated. Use B76/F80/F81 for the product RuntimeLifecycle implementation gate.
3. Qualify sealed-file receiver mounts on supported Linux systems outside WSL2, and qualify the
   sender shutdown/fencing path on each supported desktop OS. The current process supervisor is
   Linux-only and the non-Linux paths are not implemented or tested.
4. Implement and test a product sender-stop/fencing path that proves there are no stale writes
   before replacement admission. Effects and leases remain mocked in this spike; product
   Task/Attempt/Lease integration is still unimplemented.
5. Qualify an OS containment strategy for every supported desktop OS; Linux bubblewrap/PID
   namespace is the only tested process boundary so far.
6. Keep E01-S02 `IN_PROGRESS`; do not infer owner acceptance from local evidence. E01-S03
   remains blocked on its E01-S02 dependency unless the owner changes the plan explicitly.
7. Complete the E01-S01 reference-machine and hosted Actions gates.
8. Do not push without an explicit owner instruction.

## Next implementation scope

After review, continue the next dependency-ready story. Do not add UI, Operator transport,
agent adapters, cloud Runtime, RAG, or production key-service behavior to E01-S02 without a
contract/story change. Re-read `AGENTS.md`, the story, `docs/IMPLEMENTATION.md`,
`docs/STORAGE.md`, `docs/EVENTS.md`, `docs/SERVICES.md`, `docs/PROTOCOLS.md`,
`docs/NETWORK-SECURITY.md`, `docs/schemas/sqlite-v1.sql`, and relevant event schemas before
the next storage domain slice. Preserve the Task/Attempt/Effect/Evidence/Trust/fencing
invariants.
